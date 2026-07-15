use std::{fs, mem, sync::atomic::AtomicUsize};

use ::notify::{
    Config, ErrorKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use crossbeam_channel::{Receiver, Sender, select, unbounded};
use itertools::Itertools;
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};
use utils::{
    lines::LineEnding,
    paths::{AbsPath, AbsPathBuf},
    thread,
};
use walkdir::WalkDir;

use crate::loader::{self, LoadResult};

#[derive(Debug)]
pub struct NotifyHandle {
    // Relative order of fields below is significant.
    sender: Sender<ServerMsg>,
    _handler: Option<thread::JoinHandle>,
    browser_loader: Option<BrowserLoader>,
}

#[derive(Debug)]
enum ServerMsg {
    Config(loader::Config),
    Invalidate(AbsPathBuf),
}

impl loader::Handle for NotifyHandle {
    fn spawn(sender: loader::Sender) -> NotifyHandle {
        if cfg!(target_os = "emscripten") {
            let (server_sender, _) = unbounded::<ServerMsg>();
            return NotifyHandle {
                sender: server_sender,
                _handler: None,
                browser_loader: Some(BrowserLoader::new(sender)),
            };
        }

        let actor = NotifyActor::new(sender);
        let (sender, receiver) = unbounded::<ServerMsg>();
        let thread = match thread::Builder::new(thread::ThreadIntent::Worker)
            .name("VfsLoader".to_owned())
            .spawn(move || actor.run(receiver))
        {
            Ok(thread) => Some(thread),
            Err(err) => {
                tracing::error!(%err, "failed to spawn VFS loader thread");
                None
            }
        };
        NotifyHandle { sender, _handler: thread, browser_loader: None }
    }

    fn set_config(&mut self, config: loader::Config) {
        if let Some(loader) = &mut self.browser_loader {
            loader.set_config(config);
            return;
        }

        if self.sender.send(ServerMsg::Config(config)).is_err() {
            tracing::error!("failed to send VFS config to loader thread");
        }
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        if let Some(loader) = &mut self.browser_loader {
            loader.invalidate(path);
            return;
        }

        if self.sender.send(ServerMsg::Invalidate(path)).is_err() {
            tracing::error!("failed to send VFS invalidation to loader thread");
        }
    }

    fn load_sync(&mut self, path: &AbsPath) -> LoadResult {
        read(path)
    }
}

#[derive(Debug)]
struct BrowserLoader {
    sender: loader::Sender,
    config_version: u32,
    loaded_paths: FxHashSet<AbsPathBuf>,
}

impl BrowserLoader {
    fn new(sender: loader::Sender) -> Self {
        Self { sender, config_version: 0, loaded_paths: FxHashSet::default() }
    }

    fn set_config(&mut self, config: loader::Config) {
        let config_version = config.version;
        self.config_version = config_version;
        let has_reconcile_step = !self.loaded_paths.is_empty();
        let n_entries = config.to_load.len();
        let n_total = n_entries + usize::from(has_reconcile_step);
        if n_total > 0 {
            self.send(loader::Message::Progress { n_total, n_done: 0, config_version });
        }

        let previous_loaded_paths = mem::take(&mut self.loaded_paths);
        let mut reported_paths = FxHashSet::default();
        let mut loaded_paths = FxHashSet::default();
        let mut scan_failures = Vec::new();

        for (index, entry) in config.to_load.into_iter().enumerate() {
            let (watch_tx, _) = unbounded();
            let files = match NotifyActor::load_entry(&watch_tx, entry, false) {
                Ok(files) => files,
                Err(failure) => {
                    scan_failures.push(failure);
                    Vec::new()
                }
            };
            reported_paths.extend(files.iter().map(|(path, _)| path.clone()));
            loaded_paths.extend(
                files
                    .iter()
                    .filter(|(_, result)| !matches!(result, LoadResult::LoadError))
                    .map(|(path, _)| path.clone()),
            );
            self.send(loader::Message::Loaded { files, config_version });
            let n_done = index + 1;
            if n_done < n_total {
                self.send(loader::Message::Progress { n_total, n_done, config_version });
            }
        }

        if !scan_failures.is_empty() {
            loaded_paths.extend(previous_loaded_paths);
            self.loaded_paths = loaded_paths;
            for failure in scan_failures {
                self.send(loader::Message::ScanFailed { config_version, failure });
            }
            self.send(loader::Message::Progress { n_total, n_done: n_total, config_version });
            return;
        }

        let unloaded = previous_loaded_paths
            .difference(&reported_paths)
            .cloned()
            .map(|path| (path, LoadResult::LoadError))
            .collect_vec();
        self.loaded_paths = loaded_paths;
        if !unloaded.is_empty() {
            self.send(loader::Message::Loaded { files: unloaded, config_version });
        }
        self.send(loader::Message::Progress { n_total, n_done: n_total, config_version });
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        let contents = read(path.as_path());
        let files = vec![(path, contents)];
        self.record_loaded_files(&files);
        self.send(loader::Message::Changed { files, config_version: self.config_version });
    }

    fn record_loaded_files(&mut self, files: &[(AbsPathBuf, LoadResult)]) {
        for (path, result) in files {
            if matches!(result, LoadResult::LoadError) {
                self.loaded_paths.remove(path);
            } else {
                self.loaded_paths.insert(path.clone());
            }
        }
    }

    fn send(&self, msg: loader::Message) {
        if self.sender.send(msg).is_err() {
            tracing::error!("failed to send browser VFS loader message to main loop");
        }
    }
}

#[derive(Debug)]
struct WatchPlan {
    config_version: u32,
    coverage_revision: u64,
    targets: Vec<WatchTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchTarget {
    path: AbsPathBuf,
    // A path receives a new registration revision after it leaves coverage and
    // later reappears. The manager then re-registers that spelling even if the
    // OS backend silently discarded the old watch when the path was removed.
    registration_revision: u64,
}

#[derive(Debug)]
struct WatchSync {
    config_version: u32,
    coverage_revision: u64,
    targets: Vec<WatchTarget>,
}

#[derive(Debug)]
enum WatcherCommand {
    Replace(WatchPlan),
    Sync(WatchSync),
    Abort { through_config_version: u32 },
}

#[derive(Debug)]
enum WatcherOutput {
    Installed { config_version: u32, coverage_revision: u64 },
    Synced { config_version: u32, coverage_revision: u64 },
    Notify { config_version: u32, event: ::notify::Event },
    Failed { config_version: u32, failure: loader::WatcherFailure },
}

impl WatcherOutput {
    fn config_version(&self) -> u32 {
        match self {
            Self::Installed { config_version, .. }
            | Self::Synced { config_version, .. }
            | Self::Notify { config_version, .. }
            | Self::Failed { config_version, .. } => *config_version,
        }
    }
}

#[derive(Debug)]
struct WatcherManagerHandle {
    command_sender: Sender<WatcherCommand>,
    output_receiver: Receiver<WatcherOutput>,
}

#[derive(Debug, Default)]
struct WatchCoverage {
    // Desired coverage owned by the loader actor. The watcher manager owns the
    // separate installed set and only changes it from full snapshots of this map.
    targets: FxHashMap<AbsPathBuf, u64>,
    // Changes whenever the desired snapshot changes. Installation and sync
    // acknowledgements carry this value so a post-install rescan can detect gaps.
    revision: u64,
    // Monotonic identity for one incarnation of a path in desired coverage.
    next_registration_revision: u64,
}

impl WatchCoverage {
    fn replace(&mut self, paths: impl IntoIterator<Item = AbsPathBuf>) {
        self.targets.clear();
        self.revision = self.revision.wrapping_add(1);
        for path in paths {
            self.insert_new_target(path);
        }
    }

    fn add(&mut self, paths: impl IntoIterator<Item = AbsPathBuf>) -> bool {
        let mut changed = false;
        for path in paths {
            if !self.targets.contains_key(&path) {
                self.insert_new_target(path);
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }

    fn reconcile(&mut self, paths: FxHashSet<AbsPathBuf>) -> bool {
        let before = self.targets.len();
        self.targets.retain(|path, _| paths.contains(path));
        let mut changed = self.targets.len() != before;
        for path in paths {
            if !self.targets.contains_key(&path) {
                self.insert_new_target(path);
                changed = true;
            }
        }
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }

    fn invalidate_prefix(&mut self, removed_path: &AbsPath) -> bool {
        let before = self.targets.len();
        self.targets.retain(|path, _| !path.starts_with(removed_path));
        let changed = self.targets.len() != before;
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        changed
    }

    fn snapshot(&self) -> Vec<WatchTarget> {
        self.targets
            .iter()
            .map(|(path, registration_revision)| WatchTarget {
                path: path.clone(),
                registration_revision: *registration_revision,
            })
            .sorted_by(|left, right| left.path.cmp(&right.path))
            .collect()
    }

    fn insert_new_target(&mut self, path: AbsPathBuf) {
        self.next_registration_revision = self.next_registration_revision.wrapping_add(1);
        self.targets.insert(path, self.next_registration_revision);
    }
}

#[derive(Debug, Default)]
enum WatcherPhase {
    // Installing and Syncing each represent the only watcher mutation in
    // flight. Every matching acknowledgement is followed by a full rescan;
    // Ready is reached only when that rescan leaves the desired coverage at
    // the acknowledged revision. Failed is terminal for the generation.
    #[default]
    Unconfigured,
    Installing {
        coverage_revision: u64,
    },
    Syncing {
        coverage_revision: u64,
    },
    Ready,
    Failed,
}

struct NotifyActor {
    sender: loader::Sender,
    config_version: u32,
    watched_files: FxHashSet<AbsPathBuf>,
    watched_dirs: Vec<loader::Directories>,
    watch_coverage: WatchCoverage,
    watcher_phase: WatcherPhase,
    loaded_paths: FxHashSet<AbsPathBuf>,
    watcher_manager: Option<WatcherManagerHandle>,
}

#[derive(Debug)]
enum Event {
    ServerMsg(ServerMsg),
    WatcherOutput(WatcherOutput),
    WatcherManagerStopped,
}

#[derive(Debug)]
enum ActorFailure {
    Watcher(loader::WatcherFailure),
    Scan(loader::ScanFailure),
}

impl From<loader::WatcherFailure> for ActorFailure {
    fn from(failure: loader::WatcherFailure) -> Self {
        Self::Watcher(failure)
    }
}

impl From<loader::ScanFailure> for ActorFailure {
    fn from(failure: loader::ScanFailure) -> Self {
        Self::Scan(failure)
    }
}

fn spawn_watcher_manager() -> std::io::Result<WatcherManagerHandle> {
    spawn_watcher_manager_with::<RecommendedWatcher, _>(|config_version, event_sender| {
        RecommendedWatcher::new(
            move |event: ::notify::Result<::notify::Event>| {
                let output = match event {
                    Ok(event) => WatcherOutput::Notify { config_version, event },
                    Err(error) => WatcherOutput::Failed {
                        config_version,
                        failure: loader::WatcherFailure::Notify { error: error.to_string() },
                    },
                };
                if event_sender.send(output).is_err() {
                    tracing::debug!(
                        config_version,
                        "watcher event dropped because the manager receiver is closed"
                    );
                }
            },
            Config::default(),
        )
    })
}

fn spawn_watcher_manager_with<W, F>(create: F) -> std::io::Result<WatcherManagerHandle>
where
    W: Watcher + Send + 'static,
    F: FnMut(u32, Sender<WatcherOutput>) -> ::notify::Result<W> + Send + 'static,
{
    let (command_sender, command_receiver) = unbounded();
    let (output_sender, output_receiver) = unbounded();
    // OS watcher construction and registration are not cancellable and may never
    // return. This sole detached owner keeps those calls off the VFS loader and
    // main-loop shutdown paths. Commands are intentionally serialized: a stuck
    // backend call can delay later watcher generations, but it cannot delay
    // content loading or an LSP response.
    let thread = thread::Builder::new(thread::ThreadIntent::Worker)
        .name("VfsWatcherManager".to_owned())
        .allow_leak(true)
        .spawn(move || run_watcher_manager(command_receiver, output_sender, create))?;
    drop(thread);
    Ok(WatcherManagerHandle { command_sender, output_receiver })
}

// The manager is the sole owner of the OS watcher and the authoritative record
// of successfully installed targets. The actor never mutates this state
// directly.
struct ActiveWatcher<W> {
    config_version: u32,
    watcher: Option<W>,
    installed_targets: FxHashMap<AbsPathBuf, u64>,
}

fn run_watcher_manager<W, F>(
    command_receiver: Receiver<WatcherCommand>,
    output_sender: Sender<WatcherOutput>,
    mut create: F,
) where
    W: Watcher,
    F: FnMut(u32, Sender<WatcherOutput>) -> ::notify::Result<W>,
{
    let mut active: Option<ActiveWatcher<W>> = None;

    while let Ok(command) = command_receiver.recv() {
        match command {
            WatcherCommand::Replace(plan) => {
                active = None;
                let version = plan.config_version;
                let mut state = ActiveWatcher {
                    config_version: version,
                    watcher: None,
                    installed_targets: FxHashMap::default(),
                };
                if let Err(failure) = reconcile_watch_coverage(
                    version,
                    &mut state,
                    plan.targets,
                    &output_sender,
                    &mut create,
                ) {
                    if output_sender
                        .send(WatcherOutput::Failed { config_version: version, failure })
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }

                active = Some(state);
                if output_sender
                    .send(WatcherOutput::Installed {
                        config_version: version,
                        coverage_revision: plan.coverage_revision,
                    })
                    .is_err()
                {
                    return;
                }
            }
            WatcherCommand::Sync(sync) => {
                let Some(mut state) = active.take() else {
                    if output_sender
                        .send(WatcherOutput::Failed {
                            config_version: sync.config_version,
                            failure: loader::WatcherFailure::Protocol {
                                error: "watcher sync has no active generation".to_owned(),
                            },
                        })
                        .is_err()
                    {
                        return;
                    }
                    continue;
                };
                if state.config_version != sync.config_version {
                    let active_version = state.config_version;
                    active = Some(state);
                    if output_sender
                        .send(WatcherOutput::Failed {
                            config_version: sync.config_version,
                            failure: loader::WatcherFailure::Protocol {
                                error: format!(
                                    "watcher sync generation {0} does not match active generation {active_version}",
                                    sync.config_version
                                ),
                            },
                        })
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }

                if let Err(failure) = reconcile_watch_coverage(
                    sync.config_version,
                    &mut state,
                    sync.targets,
                    &output_sender,
                    &mut create,
                ) {
                    if output_sender
                        .send(WatcherOutput::Failed {
                            config_version: sync.config_version,
                            failure,
                        })
                        .is_err()
                    {
                        return;
                    }
                    continue;
                }

                active = Some(state);
                if output_sender
                    .send(WatcherOutput::Synced {
                        config_version: sync.config_version,
                        coverage_revision: sync.coverage_revision,
                    })
                    .is_err()
                {
                    return;
                }
            }
            WatcherCommand::Abort { through_config_version } => {
                let active_version = active.as_ref().map(|state| state.config_version);
                if active_version.is_some_and(|version| version <= through_config_version) {
                    active = None;
                    tracing::debug!(
                        through_config_version,
                        ?active_version,
                        "server file watcher generation aborted"
                    );
                } else {
                    tracing::debug!(
                        through_config_version,
                        ?active_version,
                        "watcher abort did not match the active generation"
                    );
                }
            }
        }
    }
}

fn reconcile_watch_coverage<W, F>(
    config_version: u32,
    state: &mut ActiveWatcher<W>,
    targets: Vec<WatchTarget>,
    output_sender: &Sender<WatcherOutput>,
    create: &mut F,
) -> Result<(), loader::WatcherFailure>
where
    W: Watcher,
    F: FnMut(u32, Sender<WatcherOutput>) -> ::notify::Result<W>,
{
    let desired_targets = targets
        .into_iter()
        .map(|target| (target.path, target.registration_revision))
        .collect::<FxHashMap<_, _>>();

    let paths_to_remove = state
        .installed_targets
        .iter()
        .filter(|(path, revision)| desired_targets.get(*path) != Some(*revision))
        .map(|(path, _)| path.clone())
        .sorted()
        .collect_vec();
    if let Some(watcher) = &mut state.watcher {
        for path in &paths_to_remove {
            tracing::debug!(config_version, %path, "server file watch removal started");
            match watcher.unwatch(path.as_ref()) {
                Ok(()) => {
                    tracing::debug!(config_version, %path, "server file watch removal finished");
                }
                Err(error)
                    if matches!(error.kind, ErrorKind::PathNotFound | ErrorKind::WatchNotFound) =>
                {
                    tracing::debug!(
                        config_version,
                        %path,
                        %error,
                        "server file watch was already removed by the backend"
                    );
                }
                Err(error) => {
                    return Err(loader::WatcherFailure::Unwatch {
                        path: path.clone(),
                        error: error.to_string(),
                    });
                }
            }
        }
    }
    for path in paths_to_remove {
        state.installed_targets.remove(&path);
    }

    let targets_to_add = desired_targets
        .iter()
        .filter(|(path, revision)| state.installed_targets.get(*path) != Some(*revision))
        .map(|(path, revision)| WatchTarget {
            path: path.clone(),
            registration_revision: *revision,
        })
        .sorted_by(|left, right| left.path.cmp(&right.path))
        .collect_vec();
    if !targets_to_add.is_empty() && state.watcher.is_none() {
        state.watcher = Some(create_watcher(config_version, output_sender, create)?);
    }
    if let Some(watcher) = &mut state.watcher {
        let paths = targets_to_add.iter().map(|target| target.path.clone()).collect_vec();
        watch_paths(config_version, watcher, &paths)?;
    }
    for target in targets_to_add {
        state.installed_targets.insert(target.path, target.registration_revision);
    }

    Ok(())
}

fn create_watcher<W, F>(
    config_version: u32,
    output_sender: &Sender<WatcherOutput>,
    create: &mut F,
) -> Result<W, loader::WatcherFailure>
where
    F: FnMut(u32, Sender<WatcherOutput>) -> ::notify::Result<W>,
{
    tracing::debug!(config_version, "server file watcher creation started");
    let watcher = create(config_version, output_sender.clone())
        .map_err(|error| loader::WatcherFailure::Create { error: error.to_string() })?;
    tracing::debug!(config_version, "server file watcher creation finished");
    Ok(watcher)
}

fn watch_paths<W: Watcher>(
    config_version: u32,
    watcher: &mut W,
    paths: &[AbsPathBuf],
) -> Result<(), loader::WatcherFailure> {
    for path in paths {
        tracing::debug!(config_version, %path, "server file watch registration started");
        if let Err(error) = watcher.watch(path.as_ref(), RecursiveMode::NonRecursive) {
            return Err(loader::WatcherFailure::Watch {
                path: path.clone(),
                error: error.to_string(),
            });
        }
        tracing::debug!(config_version, %path, "server file watch registration finished");
    }
    Ok(())
}

impl NotifyActor {
    fn new(sender: loader::Sender) -> NotifyActor {
        let watcher_manager = match spawn_watcher_manager() {
            Ok(manager) => Some(manager),
            Err(error) => {
                tracing::error!(%error, "failed to spawn server file watcher manager");
                None
            }
        };
        Self::new_with_manager(sender, watcher_manager)
    }

    fn new_with_manager(
        sender: loader::Sender,
        watcher_manager: Option<WatcherManagerHandle>,
    ) -> NotifyActor {
        NotifyActor {
            sender,
            config_version: 0,
            watched_files: FxHashSet::default(),
            watched_dirs: Vec::new(),
            watch_coverage: WatchCoverage::default(),
            watcher_phase: WatcherPhase::Unconfigured,
            loaded_paths: FxHashSet::default(),
            watcher_manager,
        }
    }

    fn next_event(&self, receiver: &Receiver<ServerMsg>) -> Option<Event> {
        let Some(watcher_manager) = &self.watcher_manager else {
            return receiver.recv().ok().map(Event::ServerMsg);
        };

        select! {
            recv(receiver) -> it => it.ok().map(Event::ServerMsg),
            recv(watcher_manager.output_receiver) -> it => Some(match it {
                Ok(output) => Event::WatcherOutput(output),
                Err(_) => Event::WatcherManagerStopped,
            }),
        }
    }

    fn run(mut self, server_inbox: Receiver<ServerMsg>) {
        while let Some(event) = self.next_event(&server_inbox) {
            tracing::debug!(?event, "vfs-loader event");
            match event {
                Event::ServerMsg(msg) => match msg {
                    ServerMsg::Config(config) => {
                        self.watcher_phase = WatcherPhase::Unconfigured;
                        if let Some(watch_plan) = self.load_and_reconcile(config) {
                            self.watcher_phase = WatcherPhase::Installing {
                                coverage_revision: watch_plan.coverage_revision,
                            };
                            self.send_watcher_command(WatcherCommand::Replace(watch_plan));
                        } else {
                            self.watcher_phase = WatcherPhase::Failed;
                        }
                    }
                    ServerMsg::Invalidate(path) => {
                        let contents = read(path.as_path());
                        let files = vec![(path, contents)];
                        self.record_loaded_files(&files);
                        self.send(loader::Message::Changed {
                            files,
                            config_version: self.config_version,
                        });
                    }
                },
                Event::WatcherOutput(output) => self.handle_watcher_output(output),
                Event::WatcherManagerStopped => {
                    self.watcher_manager = None;
                    self.fail_watcher_generation(loader::WatcherFailure::Stopped {
                        error: "server file watcher manager stopped".to_owned(),
                    });
                }
            }
        }
    }

    fn load_and_reconcile(&mut self, config: loader::Config) -> Option<WatchPlan> {
        let config_version = config.version;
        self.config_version = config_version;
        let has_reconcile_step = !self.loaded_paths.is_empty();
        let n_entries = config.to_load.len();
        let n_total = n_entries + usize::from(has_reconcile_step);
        if n_total > 0 {
            self.send(loader::Message::Progress { n_total, n_done: 0, config_version });
        }

        self.watched_files.clear();
        self.watched_dirs.clear();
        let previous_loaded_paths = mem::take(&mut self.loaded_paths);
        let (entry_tx, entry_rx) = unbounded();
        let (watch_tx, watch_rx) = unbounded();
        let (loaded_tx, loaded_rx) = unbounded();
        let (scan_failure_tx, scan_failure_rx) = unbounded();
        let processed = AtomicUsize::new(0);

        config.to_load.into_par_iter().enumerate().for_each(|(i, entry)| {
            let do_watch = config.to_watch.contains(&i);
            if do_watch && entry_tx.send(entry.clone()).is_err() {
                tracing::debug!("watched entry dropped because receiver is closed");
            }
            let files = match Self::load_entry(&watch_tx, entry, do_watch) {
                Ok(files) => files,
                Err(failure) => {
                    if scan_failure_tx.send(failure).is_err() {
                        tracing::debug!("scan failure dropped because receiver is closed");
                    }
                    Vec::new()
                }
            };
            let reported_paths = files.iter().map(|(path, _)| path.clone()).collect_vec();
            let loaded_paths = files
                .iter()
                .filter(|(_, result)| !matches!(result, LoadResult::LoadError))
                .map(|(path, _)| path.clone())
                .collect_vec();
            if loaded_tx.send((reported_paths, loaded_paths)).is_err() {
                tracing::debug!("loaded path batch dropped because receiver is closed");
            }
            self.send(loader::Message::Loaded { files, config_version });
            let n_done = 1 + processed.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            if n_done < n_total {
                self.send(loader::Message::Progress { n_total, n_done, config_version });
            }
        });
        drop(scan_failure_tx);

        drop(loaded_tx);
        let mut reported_paths = FxHashSet::default();
        let mut loaded_paths = FxHashSet::default();
        for (reported, loaded) in loaded_rx {
            reported_paths.extend(reported);
            loaded_paths.extend(loaded);
        }

        drop(entry_tx);
        for entry in entry_rx {
            match entry {
                loader::Entry::Files(files) => self.watched_files.extend(files),
                loader::Entry::Directories(dir) => self.watched_dirs.push(dir),
            }
        }

        drop(watch_tx);
        let scan_failures = scan_failure_rx.into_iter().collect_vec();
        if !scan_failures.is_empty() {
            let mut retained_paths = previous_loaded_paths;
            retained_paths.extend(loaded_paths);
            self.loaded_paths = retained_paths;
            self.fail_scan_generation(scan_failures);
            self.send(loader::Message::Progress { n_total, n_done: n_total, config_version });
            return None;
        }

        let unloaded = previous_loaded_paths
            .difference(&reported_paths)
            .cloned()
            .map(|path| (path, LoadResult::LoadError))
            .collect_vec();
        self.loaded_paths = loaded_paths;
        if !unloaded.is_empty() {
            self.send(loader::Message::Loaded { files: unloaded, config_version });
        }
        self.send(loader::Message::Progress { n_total, n_done: n_total, config_version });

        let paths: Vec<AbsPathBuf> =
            watch_rx.into_iter().collect::<FxHashSet<_>>().into_iter().sorted().collect();
        self.watch_coverage.replace(paths);
        Some(WatchPlan {
            config_version,
            coverage_revision: self.watch_coverage.revision,
            targets: self.watch_coverage.snapshot(),
        })
    }

    fn handle_watcher_output(&mut self, output: WatcherOutput) {
        if output.config_version() != self.config_version {
            return;
        }

        match output {
            WatcherOutput::Installed { config_version, coverage_revision } => {
                let expected = matches!(
                    self.watcher_phase,
                    WatcherPhase::Installing { coverage_revision: expected }
                        if expected == coverage_revision
                );
                if expected {
                    self.rescan_and_converge_watcher(config_version, coverage_revision);
                } else {
                    self.fail_watcher_generation(loader::WatcherFailure::Protocol {
                        error: format!(
                            "unexpected watcher installation acknowledgement for coverage revision {coverage_revision} while in phase {:?}",
                            self.watcher_phase
                        ),
                    });
                }
            }
            WatcherOutput::Synced { config_version, coverage_revision } => {
                let expected = matches!(
                    self.watcher_phase,
                    WatcherPhase::Syncing { coverage_revision: expected_revision }
                        if expected_revision == coverage_revision
                );
                if expected {
                    self.rescan_and_converge_watcher(config_version, coverage_revision);
                } else {
                    self.fail_watcher_generation(loader::WatcherFailure::Protocol {
                        error: format!(
                            "unexpected watcher sync acknowledgement for coverage revision {coverage_revision} while in phase {:?}",
                            self.watcher_phase
                        ),
                    });
                }
            }
            WatcherOutput::Notify { config_version, event } => {
                if matches!(self.watcher_phase, WatcherPhase::Failed) {
                    return;
                }
                let revision_before_event = self.watch_coverage.revision;
                let files = match self.process_notify_event(event) {
                    Ok(files) => files,
                    Err(failure) => {
                        self.fail_actor_generation(failure);
                        return;
                    }
                };
                self.record_loaded_files(&files);
                self.send(loader::Message::Changed { files, config_version });
                if self.watch_coverage.revision != revision_before_event
                    && matches!(self.watcher_phase, WatcherPhase::Ready)
                {
                    self.begin_watcher_sync();
                }
            }
            WatcherOutput::Failed { failure, .. } => {
                self.fail_watcher_generation(failure);
            }
        }
    }

    fn rescan_and_converge_watcher(&mut self, config_version: u32, acknowledged_revision: u64) {
        let files = match self.rescan_watched_scope() {
            Ok(files) => files,
            Err(failure) => {
                self.fail_scan_generation(vec![failure]);
                return;
            }
        };
        self.send(loader::Message::Changed { files, config_version });
        if self.watch_coverage.revision == acknowledged_revision {
            self.watcher_phase = WatcherPhase::Ready;
            self.send_watcher_status(loader::WatcherStatus::Ready { config_version });
        } else {
            self.begin_watcher_sync();
        }
    }

    fn rescan_watched_scope(
        &mut self,
    ) -> Result<Vec<(AbsPathBuf, LoadResult)>, loader::ScanFailure> {
        let mut files = self
            .watched_files
            .iter()
            .cloned()
            .map(|path| {
                let result = read(&path);
                (path, result)
            })
            .collect_vec();
        let (watch_tx, watch_rx) = unbounded();
        for file in &self.watched_files {
            let anchor = nearest_existing_parent_anchor(file)?;
            if watch_tx.send(anchor).is_err() {
                tracing::debug!("watched file anchor dropped because receiver is closed");
            }
        }
        for dirs in self.watched_dirs.clone() {
            for root in dirs.include_roots() {
                files.extend(Self::load_directory_subtree(&watch_tx, &dirs, root, true)?);
            }
        }
        drop(watch_tx);

        let reported_paths = files.iter().map(|(path, _)| path.clone()).collect::<FxHashSet<_>>();
        files.extend(
            self.loaded_paths
                .iter()
                .filter(|path| self.is_watched_file(path) && !reported_paths.contains(*path))
                .cloned()
                .map(|path| (path, LoadResult::LoadError)),
        );
        self.record_loaded_files(&files);
        self.watch_coverage.reconcile(watch_rx.into_iter().collect());
        Ok(files)
    }

    fn begin_watcher_sync(&mut self) {
        let sync = WatchSync {
            config_version: self.config_version,
            coverage_revision: self.watch_coverage.revision,
            targets: self.watch_coverage.snapshot(),
        };
        self.watcher_phase = WatcherPhase::Syncing { coverage_revision: sync.coverage_revision };
        self.send_watcher_command(WatcherCommand::Sync(sync));
    }

    fn send_watcher_command(&mut self, command: WatcherCommand) {
        let config_version = match &command {
            WatcherCommand::Replace(plan) => plan.config_version,
            WatcherCommand::Sync(sync) => sync.config_version,
            WatcherCommand::Abort { through_config_version } => *through_config_version,
        };
        let sent = self
            .watcher_manager
            .as_ref()
            .is_some_and(|manager| manager.command_sender.send(command).is_ok());
        if sent {
            return;
        }

        self.watcher_manager = None;
        debug_assert_eq!(config_version, self.config_version);
        self.fail_watcher_generation(loader::WatcherFailure::Stopped {
            error: "server file watcher manager is unavailable".to_owned(),
        });
    }

    fn fail_watcher_generation(&mut self, failure: loader::WatcherFailure) {
        if !self.enter_failed_watcher_phase() {
            return;
        }
        self.send_watcher_status(loader::WatcherStatus::Failed {
            config_version: self.config_version,
            failure,
        });
    }

    fn fail_scan_generation(&mut self, failures: Vec<loader::ScanFailure>) {
        assert!(!failures.is_empty(), "a failed scan must report at least one cause");
        if !self.enter_failed_watcher_phase() {
            return;
        }
        for failure in failures {
            self.send(loader::Message::ScanFailed { config_version: self.config_version, failure });
        }
    }

    fn fail_actor_generation(&mut self, failure: ActorFailure) {
        match failure {
            ActorFailure::Watcher(failure) => self.fail_watcher_generation(failure),
            ActorFailure::Scan(failure) => self.fail_scan_generation(vec![failure]),
        }
    }

    fn enter_failed_watcher_phase(&mut self) -> bool {
        if matches!(self.watcher_phase, WatcherPhase::Failed) {
            return false;
        }

        self.watcher_phase = WatcherPhase::Failed;
        self.abort_watcher_generation();
        true
    }

    fn abort_watcher_generation(&mut self) {
        let command = WatcherCommand::Abort { through_config_version: self.config_version };
        let sent = self
            .watcher_manager
            .as_ref()
            .is_some_and(|manager| manager.command_sender.send(command).is_ok());
        if sent || self.watcher_manager.is_none() {
            return;
        }

        tracing::error!(
            config_version = self.config_version,
            "failed to abort server file watcher generation because the manager stopped"
        );
        self.watcher_manager = None;
    }

    fn send_watcher_status(&self, status: loader::WatcherStatus) {
        self.send(loader::Message::WatcherStatus(status));
    }

    fn process_notify_event(
        &mut self,
        event: ::notify::Event,
    ) -> Result<Vec<(AbsPathBuf, LoadResult)>, ActorFailure> {
        if event.need_rescan() {
            return self.rescan_watched_scope().map_err(ActorFailure::from);
        }

        if !(event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove()) {
            return Ok(Vec::new());
        }

        let removal_path_count = match event.kind {
            kind if kind.is_remove() => event.paths.len(),
            ::notify::EventKind::Modify(ModifyKind::Name(RenameMode::From)) => event.paths.len(),
            ::notify::EventKind::Modify(ModifyKind::Name(RenameMode::Both))
                if event.paths.len() == 2 =>
            {
                1
            }
            ::notify::EventKind::Modify(ModifyKind::Name(
                RenameMode::Any | RenameMode::Other | RenameMode::Both,
            )) => return self.rescan_watched_scope().map_err(ActorFailure::from),
            _ => 0,
        };

        let mut files = Vec::new();
        for (index, raw_path) in event.paths.into_iter().enumerate() {
            let path = AbsPathBuf::try_from(raw_path.clone()).map_err(|_| {
                loader::WatcherFailure::Notify {
                    error: format!(
                        "notify returned a non-absolute or non-UTF-8 path: {raw_path:?}"
                    ),
                }
            })?;
            let is_removal_path = index < removal_path_count;
            if is_removal_path {
                self.watch_coverage.invalidate_prefix(&path);
                files.extend(self.unload_removed_path(&path));
            }

            let metadata = match fs::metadata(&path) {
                Ok(metadata) => Some(metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(loader::WatcherFailure::Notify {
                        error: format!("failed to inspect notify path {path}: {error}"),
                    }
                    .into());
                }
            };
            if is_removal_path && metadata.is_none() {
                continue;
            }
            let file_type = metadata.as_ref().map(|meta| meta.file_type());
            let is_file = file_type.as_ref().is_some_and(|it| it.is_file());
            let is_dir = file_type.as_ref().is_some_and(|it| it.is_dir());

            if is_dir && self.is_watched_dir(&path) {
                files.extend(self.load_created_directory(&path)?);
                continue;
            }

            if is_dir && self.is_ancestor_of_watched_target(&path) {
                files.extend(self.rescan_watched_scope()?);
                continue;
            }

            if metadata.is_some() && !is_file {
                continue;
            }

            if !self.is_watched_file(&path) {
                continue;
            }

            files.push((path.clone(), read(&path)));
        }

        Ok(files)
    }

    fn is_watched_dir(&self, path: &AbsPathBuf) -> bool {
        self.watched_dirs.iter().any(|dir| dir.contains_dir(path))
    }

    fn is_watched_file(&self, path: &AbsPathBuf) -> bool {
        self.watched_files.contains(path)
            || self.watched_dirs.iter().any(|dir| dir.contains_file(path))
    }

    fn is_ancestor_of_watched_target(&self, path: &AbsPath) -> bool {
        self.watched_files.iter().any(|file| file.starts_with(path))
            || self
                .watched_dirs
                .iter()
                .flat_map(|directories| directories.include_roots())
                .any(|root| root.starts_with(path))
    }

    fn load_created_directory(
        &mut self,
        path: &AbsPathBuf,
    ) -> Result<Vec<(AbsPathBuf, LoadResult)>, loader::ScanFailure> {
        let dirs =
            self.watched_dirs.iter().filter(|dir| dir.contains_dir(path)).cloned().collect_vec();
        let mut files = Vec::new();
        let mut watch_paths = FxHashSet::default();

        for dir in dirs {
            let (watch_tx, watch_rx) = unbounded();
            files.extend(Self::load_directory_subtree(&watch_tx, &dir, path, true)?);
            drop(watch_tx);
            watch_paths.extend(watch_rx);
        }

        self.watch_coverage.add(watch_paths);

        Ok(files)
    }

    fn unload_removed_path(&self, path: &AbsPathBuf) -> Vec<(AbsPathBuf, LoadResult)> {
        self.loaded_paths
            .iter()
            .filter(|loaded_path| loaded_path.starts_with(path))
            .cloned()
            .map(|path| (path, LoadResult::LoadError))
            .collect_vec()
    }

    fn load_entry(
        watch_tx: &Sender<AbsPathBuf>,
        entry: loader::Entry,
        watch: bool,
    ) -> Result<Vec<(AbsPathBuf, LoadResult)>, loader::ScanFailure> {
        match entry {
            loader::Entry::Files(files) => {
                let mut loaded = Vec::with_capacity(files.len());
                for file in files {
                    if watch {
                        let anchor = nearest_existing_parent_anchor(&file)?;
                        if watch_tx.send(anchor).is_err() {
                            tracing::debug!(
                                "watched file anchor dropped because receiver is closed"
                            );
                        }
                    }
                    let contents = read(file.as_path());
                    loaded.push((file, contents));
                }
                Ok(loaded)
            }
            loader::Entry::Directories(dirs) => {
                let mut res = Vec::new();

                for root in dirs.include_roots() {
                    res.extend(Self::load_directory_subtree(watch_tx, &dirs, root, watch)?);
                }
                Ok(res)
            }
        }
    }

    fn load_directory_subtree(
        watch_tx: &Sender<AbsPathBuf>,
        dirs: &loader::Directories,
        root: &AbsPathBuf,
        watch: bool,
    ) -> Result<Vec<(AbsPathBuf, LoadResult)>, loader::ScanFailure> {
        if watch {
            let anchor = nearest_existing_parent_anchor(root)?;
            if watch_tx.send(anchor).is_err() {
                tracing::debug!("watched directory anchor dropped because receiver is closed");
            }
        }

        let mut files = Vec::new();
        let mut walkdir = WalkDir::new(root).follow_links(true).into_iter();
        while let Some(entry) = walkdir.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) if walkdir_error_is_not_found(&error) => {
                    tracing::debug!(%error, %root, "directory disappeared while it was scanned");
                    continue;
                }
                Err(error) => return Err(scan_failure(root, error.to_string())),
            };
            let is_dir = entry.file_type().is_dir();
            let is_file = entry.file_type().is_file();
            let raw_path = entry.into_path();
            let abs_path = absolute_scan_path(root, raw_path)?;

            if is_dir && root != &abs_path && !dirs.contains_dir(&abs_path) {
                walkdir.skip_current_dir();
                continue;
            }

            if is_dir && watch && watch_tx.send(abs_path.to_owned()).is_err() {
                tracing::debug!("watched directory path dropped because receiver is closed");
            }

            if !is_file {
                continue;
            }

            if !dirs.contains_file(&abs_path) {
                continue;
            }

            let contents = read(abs_path.as_path());
            files.push((abs_path, contents));
        }

        Ok(files)
    }

    fn record_loaded_files(&mut self, files: &[(AbsPathBuf, LoadResult)]) {
        for (path, result) in files {
            if matches!(result, LoadResult::LoadError) {
                self.loaded_paths.remove(path);
            } else {
                self.loaded_paths.insert(path.clone());
            }
        }
    }

    fn send(&self, msg: loader::Message) {
        // Call self.sender with msg
        if self.sender.send(msg).is_err() {
            tracing::error!("failed to send VFS loader message to main loop");
        }
    }
}

fn nearest_existing_parent_anchor(target: &AbsPath) -> Result<AbsPathBuf, loader::ScanFailure> {
    let mut candidate = target.parent().unwrap_or(target).to_owned();
    loop {
        match fs::metadata(&candidate) {
            Ok(_) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(parent) = candidate.parent() else {
                    return Err(scan_failure(target, error.to_string()));
                };
                candidate = parent.to_owned();
            }
            Err(error) => return Err(scan_failure(target, error.to_string())),
        }
    }
}

fn walkdir_error_is_not_found(error: &walkdir::Error) -> bool {
    error.io_error().is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

fn absolute_scan_path(
    root: &AbsPath,
    raw_path: std::path::PathBuf,
) -> Result<AbsPathBuf, loader::ScanFailure> {
    AbsPathBuf::try_from(raw_path.clone()).map_err(|_| {
        scan_failure(
            root,
            format!("directory scan returned a non-absolute or non-UTF-8 path: {raw_path:?}"),
        )
    })
}

fn scan_failure(root: &AbsPath, error: String) -> loader::ScanFailure {
    loader::ScanFailure { root: root.to_owned(), error }
}

fn read(path: &AbsPath) -> LoadResult {
    let Ok(bytes) = std::fs::read(path) else {
        return LoadResult::LoadError;
    };
    let Ok(text) = String::from_utf8(bytes) else {
        return LoadResult::DecodeError;
    };
    let (text, ending) = LineEnding::normalize(text);
    LoadResult::Loaded(text, ending)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use ::notify::{
        Event as NotifyEvent, EventHandler, EventKind, WatcherKind,
        event::{CreateKind, Flag, RemoveKind},
    };
    use utils::paths::AbsPathBuf;

    use super::*;
    use crate::{
        PathMatcher,
        loader::{self, Handle as _},
    };

    struct TestDir {
        _dir: tempfile::TempDir,
        path: AbsPathBuf,
    }

    struct RejectingWatcher;

    #[derive(Default)]
    struct RecordedWatchCalls {
        watched: Vec<std::path::PathBuf>,
        unwatched: Vec<std::path::PathBuf>,
        drops: usize,
    }

    struct RecordingWatcher {
        calls: Arc<Mutex<RecordedWatchCalls>>,
    }

    impl Drop for RecordingWatcher {
        fn drop(&mut self) {
            self.calls.lock().unwrap().drops += 1;
        }
    }

    impl Watcher for RejectingWatcher {
        fn new<F: EventHandler>(_event_handler: F, _config: Config) -> ::notify::Result<Self>
        where
            Self: Sized,
        {
            unreachable!("constructed directly by watcher-manager tests")
        }

        fn watch(
            &mut self,
            _path: &std::path::Path,
            _recursive_mode: RecursiveMode,
        ) -> ::notify::Result<()> {
            Err(::notify::Error::generic("injected watch failure"))
        }

        fn unwatch(&mut self, _path: &std::path::Path) -> ::notify::Result<()> {
            Ok(())
        }

        fn kind() -> WatcherKind
        where
            Self: Sized,
        {
            WatcherKind::NullWatcher
        }
    }

    impl Watcher for RecordingWatcher {
        fn new<F: EventHandler>(_event_handler: F, _config: Config) -> ::notify::Result<Self>
        where
            Self: Sized,
        {
            unreachable!("constructed directly by watcher-manager tests")
        }

        fn watch(
            &mut self,
            path: &std::path::Path,
            _recursive_mode: RecursiveMode,
        ) -> ::notify::Result<()> {
            self.calls.lock().unwrap().watched.push(path.to_path_buf());
            Ok(())
        }

        fn unwatch(&mut self, path: &std::path::Path) -> ::notify::Result<()> {
            self.calls.lock().unwrap().unwatched.push(path.to_path_buf());
            Ok(())
        }

        fn kind() -> WatcherKind
        where
            Self: Sized,
        {
            WatcherKind::NullWatcher
        }
    }

    impl TestDir {
        fn new(name: &str) -> Self {
            let dir = tempfile::Builder::new().prefix(&format!("vide-{name}-")).tempdir().unwrap();
            let path = AbsPathBuf::assert_utf8(dir.path().to_path_buf());
            Self { _dir: dir, path }
        }

        fn join(&self, path: &str) -> AbsPathBuf {
            self.path.join(path)
        }
    }

    fn collect_until_progress_done(
        receiver: &Receiver<loader::Message>,
        version: u32,
    ) -> Vec<Vec<(AbsPathBuf, LoadResult)>> {
        let mut loaded_batches = Vec::new();
        loop {
            match receiver.recv_timeout(Duration::from_secs(1)).unwrap() {
                loader::Message::Loaded { files, config_version } if config_version == version => {
                    loaded_batches.push(files);
                }
                loader::Message::Progress { n_total, n_done, config_version }
                    if config_version == version && n_done == n_total =>
                {
                    return loaded_batches;
                }
                _ => {}
            }
        }
    }

    fn recv_version_message(receiver: &Receiver<loader::Message>, version: u32) -> loader::Message {
        loop {
            let message = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            match &message {
                loader::Message::Loaded { config_version, .. }
                | loader::Message::Changed { config_version, .. }
                | loader::Message::ScanFailed { config_version, .. }
                | loader::Message::Progress { config_version, .. }
                    if *config_version == version =>
                {
                    return message;
                }
                loader::Message::WatcherStatus(
                    loader::WatcherStatus::Ready { config_version }
                    | loader::WatcherStatus::Failed { config_version, .. },
                ) if *config_version == version => {
                    return message;
                }
                _ => {}
            }
        }
    }

    fn spawn_loader() -> (NotifyHandle, Receiver<loader::Message>) {
        let (sender, receiver) = unbounded();
        (<NotifyHandle as loader::Handle>::spawn(sender), receiver)
    }

    fn assert_loaded(batches: &[Vec<(AbsPathBuf, LoadResult)>], expected_path: &AbsPathBuf) {
        assert!(
            batches.iter().flatten().any(|(path, result)| {
                path == expected_path && matches!(result, LoadResult::Loaded(_, _))
            }),
            "expected loaded path {expected_path}, got {batches:?}"
        );
    }

    fn path_buf(path: &AbsPathBuf) -> std::path::PathBuf {
        let path: &std::path::Path = path.as_ref();
        path.to_path_buf()
    }

    fn watched_sv_dir(root: AbsPathBuf) -> loader::Directories {
        loader::Directories {
            extensions: vec!["sv".to_owned()],
            include: vec![PathMatcher::all_under_roots(vec![root])],
            exclude: Vec::new(),
            exclude_globs: None,
        }
    }

    fn actor() -> NotifyActor {
        let (sender, _receiver) = unbounded();
        NotifyActor::new(sender)
    }

    struct FakeWatcherHarness {
        server: Sender<ServerMsg>,
        loader: Receiver<loader::Message>,
        commands: Receiver<WatcherCommand>,
        output: Sender<WatcherOutput>,
        actor_thread: std::thread::JoinHandle<()>,
    }

    fn spawn_actor_with_fake_manager() -> FakeWatcherHarness {
        let (loader_sender, loader_receiver) = unbounded();
        let (command_sender, command_receiver) = unbounded();
        let (output_sender, output_receiver) = unbounded();
        let manager = WatcherManagerHandle { command_sender, output_receiver };
        let actor = NotifyActor::new_with_manager(loader_sender, Some(manager));
        let (server_sender, server_receiver) = unbounded();
        let actor_thread = std::thread::spawn(move || actor.run(server_receiver));
        FakeWatcherHarness {
            server: server_sender,
            loader: loader_receiver,
            commands: command_receiver,
            output: output_sender,
            actor_thread,
        }
    }

    fn acknowledge_command(command: &WatcherCommand, output: &Sender<WatcherOutput>) {
        let acknowledgement = match command {
            WatcherCommand::Replace(plan) => WatcherOutput::Installed {
                config_version: plan.config_version,
                coverage_revision: plan.coverage_revision,
            },
            WatcherCommand::Sync(sync) => WatcherOutput::Synced {
                config_version: sync.config_version,
                coverage_revision: sync.coverage_revision,
            },
            WatcherCommand::Abort { .. } => {
                panic!("abort commands do not have acknowledgements")
            }
        };
        output.send(acknowledgement).unwrap();
    }

    fn target_paths(targets: &[WatchTarget]) -> Vec<AbsPathBuf> {
        targets.iter().map(|target| target.path.clone()).collect()
    }

    #[test]
    fn stalled_watcher_manager_does_not_delay_content_terminal() {
        let dir = TestDir::new("vfs-loader-stalled-watcher-manager");
        let first = dir.join("first.sv");
        let second = dir.join("second.sv");
        std::fs::write(&first, "module first; endmodule\n").unwrap();
        std::fs::write(&second, "module second; endmodule\n").unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Files(vec![first.clone()])],
                to_watch: vec![0],
            }))
            .unwrap();
        assert_loaded(&collect_until_progress_done(&loader, 1), &first);
        assert!(matches!(
            commands.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherCommand::Replace(WatchPlan { config_version: 1, .. })
        ));

        server
            .send(ServerMsg::Config(loader::Config {
                version: 2,
                to_load: vec![loader::Entry::Files(vec![second.clone()])],
                to_watch: vec![0],
            }))
            .unwrap();
        assert_loaded(&collect_until_progress_done(&loader, 2), &second);
        assert!(matches!(
            commands.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherCommand::Replace(WatchPlan { config_version: 2, .. })
        ));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn watcher_install_ack_rescans_scan_install_gap() {
        let dir = TestDir::new("vfs-loader-watch-install-gap");
        let root = dir.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root.clone()))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let WatcherCommand::Replace(plan) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected watcher replacement");
        };
        let initial_targets = target_paths(&plan.targets);
        assert!(initial_targets.contains(&root));
        assert!(initial_targets.contains(&root.parent().unwrap().to_owned()));

        let created_dir = root.join("generated");
        let created_file = created_dir.join("new.sv");
        std::fs::create_dir_all(&created_dir).unwrap();
        std::fs::write(&created_file, "module new; endmodule\n").unwrap();
        output
            .send(WatcherOutput::Installed {
                config_version: 1,
                coverage_revision: plan.coverage_revision,
            })
            .unwrap();
        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected gap-closing rescan");
        };
        assert!(files.iter().any(|(path, result)| {
            path == &created_file
                && matches!(result, LoadResult::Loaded(text, _) if text.contains("module new"))
        }));
        let WatcherCommand::Sync(sync) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected watches for directories created during installation");
        };
        assert_eq!(sync.config_version, 1);
        assert!(target_paths(&sync.targets).contains(&created_dir));

        acknowledge_command(&WatcherCommand::Sync(sync), &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn unexpected_watcher_ack_fails_the_generation() {
        let dir = TestDir::new("vfs-loader-unexpected-watcher-ack");
        let root = dir.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let WatcherCommand::Replace(plan) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected watcher replacement");
        };

        output
            .send(WatcherOutput::Installed {
                config_version: 1,
                coverage_revision: plan.coverage_revision + 1,
            })
            .unwrap();

        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Failed {
                config_version: 1,
                failure: loader::WatcherFailure::Protocol { error },
            }) if error.contains("unexpected watcher installation acknowledgement")
        ));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn watcher_failure_is_terminal_for_the_current_generation() {
        let dir = TestDir::new("vfs-loader-terminal-watcher-failure");
        let root = dir.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root.clone()))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();

        let created_dir = root.join("generated");
        std::fs::create_dir_all(&created_dir).unwrap();
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        let sync = commands.recv_timeout(Duration::from_secs(1)).unwrap();

        output
            .send(WatcherOutput::Failed {
                config_version: 1,
                failure: loader::WatcherFailure::Notify { error: "backend failed".to_owned() },
            })
            .unwrap();
        acknowledge_command(&sync, &output);

        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Failed {
                config_version: 1,
                failure: loader::WatcherFailure::Notify { error },
            }) if error == "backend failed"
        ));
        while let Ok(message) = loader.recv_timeout(Duration::from_millis(100)) {
            assert!(
                !matches!(
                    message,
                    loader::Message::WatcherStatus(loader::WatcherStatus::Ready {
                        config_version: 1
                    })
                ),
                "failed watcher generation must never become ready"
            );
        }

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn rescan_flag_reconciles_missed_files_and_watch_coverage() {
        let dir = TestDir::new("vfs-loader-rescan-flag");
        let root = dir.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root.clone()))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        let missed_dir = root.join("generated");
        let missed_file = missed_dir.join("new.sv");
        std::fs::create_dir_all(&missed_dir).unwrap();
        std::fs::write(&missed_file, "module new; endmodule\n").unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Other).set_flag(Flag::Rescan),
            })
            .unwrap();

        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected a full rescan after the backend rescan flag");
        };
        assert!(files.iter().any(|(path, result)| {
            path == &missed_file
                && matches!(result, LoadResult::Loaded(text, _) if text.contains("module new"))
        }));
        let WatcherCommand::Sync(sync) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected missed directory coverage to be synchronized");
        };
        assert!(target_paths(&sync.targets).contains(&missed_dir));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn rename_from_unloads_directory_and_removes_watch_coverage() {
        let dir = TestDir::new("vfs-loader-rename-from");
        let root = dir.join("workspace");
        let moved_from = root.join("generated");
        let moved_file = moved_from.join("old.sv");
        let moved_to = dir.join("outside");
        std::fs::create_dir_all(&moved_from).unwrap();
        std::fs::write(&moved_file, "module old; endmodule\n").unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root))],
                to_watch: vec![0],
            }))
            .unwrap();
        assert_loaded(&collect_until_progress_done(&loader, 1), &moved_file);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        std::fs::rename(&moved_from, &moved_to).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Modify(ModifyKind::Name(RenameMode::From)))
                    .add_path(path_buf(&moved_from)),
            })
            .unwrap();

        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected moved directory contents to be unloaded");
        };
        assert_eq!(files, vec![(moved_file, LoadResult::LoadError)]);
        let WatcherCommand::Sync(sync) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected moved directory coverage to be removed");
        };
        assert!(!target_paths(&sync.targets).contains(&moved_from));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn watcher_sync_has_only_one_mutation_in_flight() {
        let dir = TestDir::new("vfs-loader-single-watch-batch");
        let root = dir.join("workspace");
        std::fs::create_dir_all(&root).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root.clone()))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        let first = root.join("first");
        std::fs::create_dir_all(&first).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                    .add_path(path_buf(&first)),
            })
            .unwrap();
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        let first_command = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        let WatcherCommand::Sync(first_sync) = &first_command else {
            panic!("expected first full coverage sync");
        };
        assert_eq!(first_sync.config_version, 1);
        assert!(target_paths(&first_sync.targets).contains(&first));

        let second = root.join("second");
        std::fs::create_dir_all(&second).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                    .add_path(path_buf(&second)),
            })
            .unwrap();
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(
            commands.recv_timeout(Duration::from_millis(100)).is_err(),
            "a second coverage sync was sent before the first was acknowledged"
        );

        acknowledge_command(&first_command, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        let WatcherCommand::Sync(second_sync) =
            commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected the pending coverage sync after the first acknowledgement");
        };
        assert!(target_paths(&second_sync.targets).contains(&second));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn recreated_directory_is_registered_again() {
        let dir = TestDir::new("vfs-loader-recreated-watch-directory");
        let root = dir.join("workspace");
        let recreated = root.join("generated");
        std::fs::create_dir_all(&root).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root.clone()))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        std::fs::create_dir_all(&recreated).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                    .add_path(path_buf(&recreated)),
            })
            .unwrap();
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        let initial_add = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        acknowledge_command(&initial_add, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        std::fs::remove_dir(&recreated).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Remove(RemoveKind::Folder))
                    .add_path(path_buf(&recreated)),
            })
            .unwrap();
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        let removal_sync = commands.recv_timeout(Duration::from_secs(1)).unwrap();

        std::fs::create_dir_all(&recreated).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                    .add_path(path_buf(&recreated)),
            })
            .unwrap();
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(commands.recv_timeout(Duration::from_millis(100)).is_err());
        acknowledge_command(&removal_sync, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        let WatcherCommand::Sync(reinstall_sync) =
            commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected recreated directory coverage sync");
        };
        let recreated_target = reinstall_sync
            .targets
            .iter()
            .find(|target| target.path == recreated)
            .expect("recreated directory should be in coverage");
        let original_revision = match initial_add {
            WatcherCommand::Sync(sync) => {
                sync.targets
                    .into_iter()
                    .find(|target| target.path == recreated)
                    .unwrap()
                    .registration_revision
            }
            WatcherCommand::Replace(_) | WatcherCommand::Abort { .. } => unreachable!(),
        };
        assert_ne!(recreated_target.registration_revision, original_revision);

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directory_walk_errors_are_all_reported_before_terminal_progress() {
        let dir = TestDir::new("vfs-loader-directory-walk-errors");
        let first_root = dir.join("first-workspace");
        let second_root = dir.join("second-workspace");
        for root in [&first_root, &second_root] {
            std::fs::create_dir_all(root).unwrap();
            std::os::unix::fs::symlink(root, root.join("loop")).unwrap();
        }
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![
                    loader::Entry::Directories(watched_sv_dir(first_root.clone())),
                    loader::Entry::Directories(watched_sv_dir(second_root.clone())),
                ],
                to_watch: vec![0, 1],
            }))
            .unwrap();
        let mut failed_roots = FxHashSet::default();
        loop {
            match recv_version_message(&loader, 1) {
                loader::Message::ScanFailed {
                    config_version: 1,
                    failure: loader::ScanFailure { root: failed_root, error },
                } => {
                    assert!(!error.is_empty());
                    failed_roots.insert(failed_root);
                }
                loader::Message::Progress { n_done, n_total, .. } if n_done == n_total => {
                    assert_eq!(
                        failed_roots,
                        FxHashSet::from_iter([first_root.clone(), second_root.clone()]),
                        "every scan failure must be reported before terminal progress"
                    );
                    break;
                }
                _ => {}
            }
        }
        assert!(matches!(
            commands.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherCommand::Abort { through_config_version: 1 }
        ));
        assert!(commands.recv_timeout(Duration::from_millis(100)).is_err());

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn new_generation_scan_failure_aborts_previous_watcher() {
        let dir = TestDir::new("vfs-loader-new-scan-failure-abort");
        let initial_file = dir.join("initial.sv");
        std::fs::write(&initial_file, "module initial; endmodule\n").unwrap();
        let failed_root = dir.join("failed-workspace");
        std::fs::create_dir_all(&failed_root).unwrap();
        std::os::unix::fs::symlink(&failed_root, failed_root.join("loop")).unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Files(vec![initial_file])],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        server
            .send(ServerMsg::Config(loader::Config {
                version: 2,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(failed_root))],
                to_watch: vec![0],
            }))
            .unwrap();
        let mut failure_seen = false;
        loop {
            match recv_version_message(&loader, 2) {
                loader::Message::ScanFailed { .. } => failure_seen = true,
                loader::Message::Progress { n_done, n_total, .. } if n_done == n_total => {
                    assert!(failure_seen);
                    break;
                }
                _ => {}
            }
        }
        assert!(matches!(
            commands.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherCommand::Abort { through_config_version: 2 }
        ));
        assert!(commands.recv_timeout(Duration::from_millis(100)).is_err());

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn invalid_walk_path_is_a_structured_scan_failure() {
        let dir = TestDir::new("vfs-loader-invalid-walk-path");
        let root = dir.join("workspace");
        let failure = absolute_scan_path(&root, std::path::PathBuf::from("relative")).unwrap_err();
        assert!(matches!(
            failure,
            loader::ScanFailure { root: failed_root, error }
                if failed_root == root && error.contains("relative")
        ));
    }

    #[test]
    fn manager_reregisters_a_recreated_path_with_a_new_revision() {
        let dir = TestDir::new("vfs-loader-manager-reregister");
        let watched_path = dir.join("generated");
        std::fs::create_dir_all(&watched_path).unwrap();
        let calls = Arc::new(Mutex::new(RecordedWatchCalls::default()));
        let manager_calls = Arc::clone(&calls);
        let manager = spawn_watcher_manager_with::<RecordingWatcher, _>(move |_, _| {
            Ok(RecordingWatcher { calls: Arc::clone(&manager_calls) })
        })
        .unwrap();

        manager
            .command_sender
            .send(WatcherCommand::Replace(WatchPlan {
                config_version: 1,
                coverage_revision: 1,
                targets: vec![WatchTarget { path: watched_path.clone(), registration_revision: 1 }],
            }))
            .unwrap();
        assert!(matches!(
            manager.output_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherOutput::Installed { config_version: 1, coverage_revision: 1 }
        ));

        manager
            .command_sender
            .send(WatcherCommand::Sync(WatchSync {
                config_version: 1,
                coverage_revision: 2,
                targets: vec![WatchTarget { path: watched_path.clone(), registration_revision: 2 }],
            }))
            .unwrap();
        assert!(matches!(
            manager.output_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherOutput::Synced { config_version: 1, coverage_revision: 2 }
        ));

        let calls = calls.lock().unwrap();
        assert_eq!(calls.watched, vec![path_buf(&watched_path), path_buf(&watched_path)]);
        assert_eq!(calls.unwatched, vec![path_buf(&watched_path)]);
    }

    #[test]
    fn manager_abort_drops_an_older_active_generation() {
        let dir = TestDir::new("vfs-loader-manager-abort");
        let watched_path = dir.join("workspace");
        std::fs::create_dir_all(&watched_path).unwrap();
        let calls = Arc::new(Mutex::new(RecordedWatchCalls::default()));
        let manager_calls = Arc::clone(&calls);
        let manager = spawn_watcher_manager_with::<RecordingWatcher, _>(move |_, _| {
            Ok(RecordingWatcher { calls: Arc::clone(&manager_calls) })
        })
        .unwrap();

        manager
            .command_sender
            .send(WatcherCommand::Replace(WatchPlan {
                config_version: 1,
                coverage_revision: 1,
                targets: vec![WatchTarget { path: watched_path, registration_revision: 1 }],
            }))
            .unwrap();
        assert!(matches!(
            manager.output_receiver.recv_timeout(Duration::from_secs(1)).unwrap(),
            WatcherOutput::Installed { config_version: 1, coverage_revision: 1 }
        ));

        manager.command_sender.send(WatcherCommand::Abort { through_config_version: 2 }).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while calls.lock().unwrap().drops == 0 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert_eq!(calls.lock().unwrap().drops, 1);
    }

    #[test]
    fn exact_file_uses_parent_anchor_across_delete_and_recreate() {
        let dir = TestDir::new("vfs-loader-exact-file-anchor");
        let file = dir.join("top.sv");
        std::fs::write(&file, "module first; endmodule\n").unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Files(vec![file.clone()])],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        let WatcherCommand::Replace(plan) = &install else {
            panic!("expected initial watcher plan");
        };
        assert_eq!(target_paths(&plan.targets), vec![file.parent().unwrap().to_owned()]);
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        std::fs::remove_file(&file).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Remove(RemoveKind::File))
                    .add_path(path_buf(&file)),
            })
            .unwrap();
        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected exact file unload");
        };
        assert_eq!(files, vec![(file.clone(), LoadResult::LoadError)]);

        std::fs::write(&file, "module second; endmodule\n").unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Create(CreateKind::File))
                    .add_path(path_buf(&file)),
            })
            .unwrap();
        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected recreated exact file load");
        };
        assert!(matches!(
            files.as_slice(),
            [(path, LoadResult::Loaded(text, _))]
                if path == &file && text.contains("module second")
        ));
        assert!(commands.recv_timeout(Duration::from_millis(100)).is_err());

        std::fs::remove_file(&file).unwrap();
        std::fs::write(&file, "module third; endmodule\n").unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Remove(RemoveKind::File))
                    .add_path(path_buf(&file)),
            })
            .unwrap();
        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected delayed remove to observe the recreated exact file");
        };
        assert!(
            files
                .iter()
                .any(|(path, result)| { path == &file && matches!(result, LoadResult::LoadError) })
        );
        assert!(matches!(
            files.iter().rev().find(|(path, _)| path == &file),
            Some((_, LoadResult::Loaded(text, _))) if text.contains("module third")
        ));
        assert!(commands.recv_timeout(Duration::from_millis(100)).is_err());

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn directory_root_uses_parent_anchor_across_delete_and_recreate() {
        let dir = TestDir::new("vfs-loader-directory-root-anchor");
        let root = dir.join("workspace");
        let file = root.join("top.sv");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&file, "module first; endmodule\n").unwrap();
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Directories(watched_sv_dir(root.clone()))],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        let install = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        let WatcherCommand::Replace(plan) = &install else {
            panic!("expected initial watcher plan");
        };
        assert!(target_paths(&plan.targets).contains(&root.parent().unwrap().to_owned()));
        acknowledge_command(&install, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        std::fs::remove_dir_all(&root).unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Remove(RemoveKind::Folder))
                    .add_path(path_buf(&root)),
            })
            .unwrap();
        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected directory root unload");
        };
        assert_eq!(files, vec![(file.clone(), LoadResult::LoadError)]);
        let removal_sync = commands.recv_timeout(Duration::from_secs(1)).unwrap();
        let WatcherCommand::Sync(sync) = &removal_sync else {
            panic!("expected root removal coverage sync");
        };
        assert!(!target_paths(&sync.targets).contains(&root));
        assert!(target_paths(&sync.targets).contains(&root.parent().unwrap().to_owned()));
        acknowledge_command(&removal_sync, &output);
        assert!(matches!(recv_version_message(&loader, 1), loader::Message::Changed { .. }));
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Ready { config_version: 1 })
        ));

        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&file, "module second; endmodule\n").unwrap();
        output
            .send(WatcherOutput::Notify {
                config_version: 1,
                event: NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                    .add_path(path_buf(&root)),
            })
            .unwrap();
        let loader::Message::Changed { files, .. } = recv_version_message(&loader, 1) else {
            panic!("expected recreated directory root load");
        };
        assert!(files.iter().any(|(path, result)| {
            path == &file
                && matches!(result, LoadResult::Loaded(text, _) if text.contains("module second"))
        }));
        let WatcherCommand::Sync(sync) = commands.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected recreated directory root coverage sync");
        };
        assert!(target_paths(&sync.targets).contains(&root));

        drop(server);
        drop(output);
        actor_thread.join().unwrap();
    }

    #[test]
    fn watcher_failure_is_reported_structurally() {
        let dir = TestDir::new("vfs-loader-watcher-failure");
        let file = dir.join("top.sv");
        std::fs::write(&file, "module top; endmodule\n").unwrap();
        let manager = spawn_watcher_manager_with::<RejectingWatcher, _>(|_, _| {
            Err(::notify::Error::generic("injected creation failure"))
        })
        .unwrap();
        let (loader_sender, loader) = unbounded();
        let actor = NotifyActor::new_with_manager(loader_sender, Some(manager));
        let (server, server_receiver) = unbounded();
        let actor_thread = std::thread::spawn(move || actor.run(server_receiver));

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Files(vec![file])],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);

        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Failed {
                config_version: 1,
                failure: loader::WatcherFailure::Create { error },
            }) if error == "injected creation failure"
        ));

        drop(server);
        actor_thread.join().unwrap();
    }

    #[test]
    fn watcher_path_failure_drops_generation_and_is_reported_structurally() {
        let dir = TestDir::new("vfs-loader-watch-path-failure");
        let file = dir.join("top.sv");
        std::fs::write(&file, "module top; endmodule\n").unwrap();
        let manager =
            spawn_watcher_manager_with::<RejectingWatcher, _>(|_, _| Ok(RejectingWatcher)).unwrap();
        let (loader_sender, loader) = unbounded();
        let actor = NotifyActor::new_with_manager(loader_sender, Some(manager));
        let (server, server_receiver) = unbounded();
        let actor_thread = std::thread::spawn(move || actor.run(server_receiver));

        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: vec![loader::Entry::Files(vec![file.clone()])],
                to_watch: vec![0],
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);

        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Failed {
                config_version: 1,
                failure: loader::WatcherFailure::Watch { path, error },
            }) if path.as_path() == file.parent().unwrap()
                && error == "injected watch failure"
        ));

        drop(server);
        actor_thread.join().unwrap();
    }

    #[test]
    fn stopped_watcher_manager_is_reported_structurally() {
        let FakeWatcherHarness { server, loader, commands, output, actor_thread } =
            spawn_actor_with_fake_manager();
        server
            .send(ServerMsg::Config(loader::Config {
                version: 1,
                to_load: Vec::new(),
                to_watch: Vec::new(),
            }))
            .unwrap();
        collect_until_progress_done(&loader, 1);
        commands.recv_timeout(Duration::from_secs(1)).unwrap();

        drop(output);
        assert!(matches!(
            recv_version_message(&loader, 1),
            loader::Message::WatcherStatus(loader::WatcherStatus::Failed {
                config_version: 1,
                failure: loader::WatcherFailure::Stopped { error },
            }) if error == "server file watcher manager stopped"
        ));

        drop(server);
        actor_thread.join().unwrap();
    }

    #[test]
    fn empty_config_emits_ready_ack_progress() {
        let (mut handle, receiver) = spawn_loader();

        handle.set_config(loader::Config { version: 1, to_load: Vec::new(), to_watch: Vec::new() });

        assert!(matches!(
            recv_version_message(&receiver, 1),
            loader::Message::Progress { n_done: 0, n_total: 0, .. }
        ));
    }

    #[test]
    fn removed_config_file_is_unloaded() {
        let dir = TestDir::new("vfs-loader-unload-file");
        let file = dir.join("top.sv");
        std::fs::write(&file, "module top; endmodule\n").unwrap();
        let (mut handle, receiver) = spawn_loader();

        handle.set_config(loader::Config {
            version: 1,
            to_load: vec![loader::Entry::Files(vec![file.clone()])],
            to_watch: Vec::new(),
        });
        let loaded = collect_until_progress_done(&receiver, 1);
        assert_loaded(&loaded, &file);

        handle.set_config(loader::Config { version: 2, to_load: Vec::new(), to_watch: Vec::new() });

        assert!(matches!(
            recv_version_message(&receiver, 2),
            loader::Message::Progress { n_done: 0, n_total: 1, .. }
        ));
        let loader::Message::Loaded { files: unloaded, .. } = recv_version_message(&receiver, 2)
        else {
            panic!("expected unload batch before final progress");
        };
        assert_eq!(unloaded, vec![(file, LoadResult::LoadError)]);
        assert!(matches!(
            recv_version_message(&receiver, 2),
            loader::Message::Progress { n_done: 1, n_total: 1, .. }
        ));
    }

    #[test]
    fn configured_missing_file_is_not_reconciled_twice() {
        let dir = TestDir::new("vfs-loader-missing-config-file");
        let file = dir.join("top.sv");
        std::fs::write(&file, "module top; endmodule\n").unwrap();
        let (mut handle, receiver) = spawn_loader();

        handle.set_config(loader::Config {
            version: 1,
            to_load: vec![loader::Entry::Files(vec![file.clone()])],
            to_watch: Vec::new(),
        });
        let loaded = collect_until_progress_done(&receiver, 1);
        assert_loaded(&loaded, &file);

        std::fs::remove_file(&file).unwrap();
        handle.set_config(loader::Config {
            version: 2,
            to_load: vec![loader::Entry::Files(vec![file.clone()])],
            to_watch: Vec::new(),
        });

        let mut unload_count = 0;
        loop {
            match recv_version_message(&receiver, 2) {
                loader::Message::Loaded { files, .. } => {
                    unload_count += files
                        .iter()
                        .filter(|(path, result)| {
                            path == &file && matches!(result, LoadResult::LoadError)
                        })
                        .count();
                }
                loader::Message::Progress { n_done, n_total, .. } if n_done == n_total => break,
                _ => {}
            }
        }

        assert_eq!(unload_count, 1);
    }

    #[test]
    fn removed_config_directory_is_unloaded() {
        let dir = TestDir::new("vfs-loader-unload-directory");
        let source_dir = dir.join("rtl");
        std::fs::create_dir_all(&source_dir).unwrap();
        let file = source_dir.join("top.sv");
        std::fs::write(&file, "module top; endmodule\n").unwrap();
        let (mut handle, receiver) = spawn_loader();

        handle.set_config(loader::Config {
            version: 1,
            to_load: vec![loader::Entry::Directories(loader::Directories {
                extensions: vec!["sv".to_owned()],
                include: vec![PathMatcher::all_under_roots(vec![source_dir])],
                exclude: Vec::new(),
                exclude_globs: None,
            })],
            to_watch: Vec::new(),
        });
        let loaded = collect_until_progress_done(&receiver, 1);
        assert_loaded(&loaded, &file);

        handle.set_config(loader::Config { version: 2, to_load: Vec::new(), to_watch: Vec::new() });

        assert!(matches!(
            recv_version_message(&receiver, 2),
            loader::Message::Progress { n_done: 0, n_total: 1, .. }
        ));
        let loader::Message::Loaded { files: unloaded, .. } = recv_version_message(&receiver, 2)
        else {
            panic!("expected unload batch before final progress");
        };
        assert_eq!(unloaded, vec![(file, LoadResult::LoadError)]);
        assert!(matches!(
            recv_version_message(&receiver, 2),
            loader::Message::Progress { n_done: 1, n_total: 1, .. }
        ));
    }

    #[test]
    fn created_watched_directory_is_loaded_immediately() {
        let dir = TestDir::new("vfs-loader-created-directory-load");
        let root = dir.join("workspace");
        let created_dir = root.join("generated");
        let nested_dir = created_dir.join("nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let top = created_dir.join("top.sv");
        let child = nested_dir.join("child.sv");
        let ignored = created_dir.join("notes.txt");
        std::fs::write(&top, "module top; endmodule\n").unwrap();
        std::fs::write(&child, "module child; endmodule\n").unwrap();
        std::fs::write(&ignored, "not systemverilog").unwrap();
        let mut actor = actor();
        actor.watched_dirs.push(watched_sv_dir(root));

        let files = actor
            .process_notify_event(
                NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                    .add_path(path_buf(&created_dir)),
            )
            .unwrap();
        actor.record_loaded_files(&files);

        assert!(
            files.iter().any(|(path, result)| {
                path == &top && matches!(result, LoadResult::Loaded(_, _))
            })
        );
        assert!(files.iter().any(|(path, result)| {
            path == &child && matches!(result, LoadResult::Loaded(_, _))
        }));
        assert!(!files.iter().any(|(path, _)| path == &ignored));
        assert!(actor.loaded_paths.contains(&top));
        assert!(actor.loaded_paths.contains(&child));
    }

    #[test]
    fn removed_watched_directory_unloads_loaded_descendants() {
        let dir = TestDir::new("vfs-loader-removed-directory-unload");
        let root = dir.join("workspace");
        let removed_dir = root.join("removed");
        let top = removed_dir.join("top.sv");
        let child = removed_dir.join("nested/child.sv");
        let sibling = root.join("sibling.sv");
        let mut actor = actor();
        actor.watched_dirs.push(watched_sv_dir(root));
        actor.loaded_paths.extend([top.clone(), child.clone(), sibling.clone()]);

        let mut files = actor
            .process_notify_event(
                NotifyEvent::new(EventKind::Remove(RemoveKind::Folder))
                    .add_path(path_buf(&removed_dir)),
            )
            .unwrap();
        files.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
        actor.record_loaded_files(&files);

        let mut expected =
            vec![(child.clone(), LoadResult::LoadError), (top.clone(), LoadResult::LoadError)];
        expected.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
        assert_eq!(files, expected);
        assert!(!actor.loaded_paths.contains(&top));
        assert!(!actor.loaded_paths.contains(&child));
        assert!(actor.loaded_paths.contains(&sibling));
    }
}
