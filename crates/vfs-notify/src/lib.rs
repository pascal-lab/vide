use std::{fs, mem, sync::atomic::AtomicUsize};

use crossbeam_channel::{Receiver, Sender, select, unbounded};
use itertools::Itertools;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};
use rustc_hash::FxHashSet;
use utils::{
    lines::LineEnding,
    paths::{AbsPath, AbsPathBuf},
    thread,
};
use vfs::loader::{self, LoadResult};
use walkdir::WalkDir;

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

        for (index, entry) in config.to_load.into_iter().enumerate() {
            let (watch_tx, _) = unbounded();
            let files = NotifyActor::load_entry(&watch_tx, entry, false);
            reported_paths.extend(files.iter().map(|(path, _)| path.clone()));
            loaded_paths.extend(
                files
                    .iter()
                    .filter(|(_, result)| !matches!(result, LoadResult::LoadError))
                    .map(|(path, _)| path.clone()),
            );
            self.send(loader::Message::Loaded { files, config_version });
            self.send(loader::Message::Progress { n_total, n_done: index + 1, config_version });
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
        if has_reconcile_step {
            self.send(loader::Message::Progress { n_total, n_done: n_total, config_version });
        } else if n_total == 0 {
            self.send(loader::Message::Progress { n_total, n_done: 0, config_version });
        }
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        let contents = read(path.as_path());
        let files = vec![(path, contents)];
        self.record_loaded_files(&files);
        self.send(loader::Message::Loaded { files, config_version: self.config_version });
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

type NotifyEvent = notify::Result<notify::Event>;

struct NotifyActor {
    sender: loader::Sender,
    config_version: u32,
    watched_files: FxHashSet<AbsPathBuf>,
    watched_dirs: Vec<loader::Directories>,
    loaded_paths: FxHashSet<AbsPathBuf>,
    // Drop order is significant.
    watcher: Option<(RecommendedWatcher, Receiver<NotifyEvent>)>,
}

#[derive(Debug)]
enum Event {
    ServerMsg(ServerMsg),
    NotifyEvent(NotifyEvent),
}

impl NotifyActor {
    fn new(sender: loader::Sender) -> NotifyActor {
        NotifyActor {
            sender,
            config_version: 0,
            watched_files: FxHashSet::default(),
            watched_dirs: Vec::new(),
            loaded_paths: FxHashSet::default(),
            watcher: None,
        }
    }

    fn next_event(&self, receiver: &Receiver<ServerMsg>) -> Option<Event> {
        let Some((_, watcher_receiver)) = &self.watcher else {
            return receiver.recv().ok().map(Event::ServerMsg);
        };

        select! {
            recv(receiver) -> it => it.ok().map(Event::ServerMsg),
            recv(watcher_receiver) -> it => it.ok().map(Event::NotifyEvent),
        }
    }

    fn run(mut self, server_inbox: Receiver<ServerMsg>) {
        while let Some(event) = self.next_event(&server_inbox) {
            tracing::debug!(?event, "vfs-notify event");
            match event {
                Event::ServerMsg(msg) => match msg {
                    ServerMsg::Config(config) => {
                        self.watcher = None;
                        if !config.to_watch.is_empty() {
                            let (watcher_sender, watcher_receiver) = unbounded();
                            let watcher = log_notify_error(RecommendedWatcher::new(
                                move |event| {
                                    if watcher_sender.send(event).is_err() {
                                        tracing::debug!(
                                            "notify event dropped because watcher receiver is closed"
                                        );
                                    }
                                },
                                Config::default(),
                            ));
                            self.watcher = watcher.map(|it| (it, watcher_receiver));
                        }

                        let config_version = config.version;
                        self.config_version = config_version;
                        let has_reconcile_step = !self.loaded_paths.is_empty();
                        let n_entries = config.to_load.len();
                        let n_total = n_entries + usize::from(has_reconcile_step);
                        if n_total > 0 {
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: 0,
                                config_version,
                            });
                        }

                        self.watched_files.clear();
                        self.watched_dirs.clear();
                        let previous_loaded_paths = mem::take(&mut self.loaded_paths);

                        let (entry_tx, entry_rx) = unbounded();
                        let (watch_tx, watch_rx) = unbounded();
                        let (loaded_tx, loaded_rx) = unbounded();
                        let processed = AtomicUsize::new(0);

                        config.to_load.into_par_iter().enumerate().for_each(|(i, entry)| {
                            let do_watch = config.to_watch.contains(&i);
                            if do_watch && entry_tx.send(entry.clone()).is_err() {
                                tracing::debug!("watched entry dropped because receiver is closed");
                            }
                            let files = Self::load_entry(&watch_tx, entry, do_watch);
                            let reported_paths =
                                files.iter().map(|(path, _)| path.clone()).collect_vec();
                            let loaded_paths = files
                                .iter()
                                .filter(|(_, result)| !matches!(result, LoadResult::LoadError))
                                .map(|(path, _)| path.clone())
                                .collect_vec();
                            if loaded_tx.send((reported_paths, loaded_paths)).is_err() {
                                tracing::debug!(
                                    "loaded path batch dropped because receiver is closed"
                                );
                            }
                            self.send(loader::Message::Loaded { files, config_version });
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: 1 + processed
                                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel),
                                config_version,
                            });
                        });

                        drop(loaded_tx);
                        let mut reported_paths = FxHashSet::default();
                        let mut loaded_paths = FxHashSet::default();
                        for (reported, loaded) in loaded_rx {
                            reported_paths.extend(reported);
                            loaded_paths.extend(loaded);
                        }

                        drop(watch_tx);
                        for path in watch_rx {
                            self.watch(&path);
                        }

                        drop(entry_tx);
                        for entry in entry_rx {
                            match entry {
                                loader::Entry::Files(files) => self.watched_files.extend(files),
                                loader::Entry::Directories(dir) => self.watched_dirs.push(dir),
                            }
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
                        if has_reconcile_step {
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: n_total,
                                config_version,
                            });
                        } else if n_total == 0 {
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: 0,
                                config_version,
                            });
                        }
                    }
                    ServerMsg::Invalidate(path) => {
                        let contents = read(path.as_path());
                        let files = vec![(path, contents)];
                        self.record_loaded_files(&files);
                        self.send(loader::Message::Loaded {
                            files,
                            config_version: self.config_version,
                        });
                    }
                },
                Event::NotifyEvent(event) => {
                    let Some(event) = log_notify_error(event) else {
                        continue;
                    };

                    let files = self.process_notify_event(event);
                    self.record_loaded_files(&files);
                    self.send(loader::Message::Loaded {
                        files,
                        config_version: self.config_version,
                    });
                }
            }
        }
    }

    fn process_notify_event(&mut self, event: notify::Event) -> Vec<(AbsPathBuf, LoadResult)> {
        if !(event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove()) {
            return Vec::new();
        }

        let mut files = Vec::new();
        for path in event.paths.into_iter().filter_map(|path| AbsPathBuf::try_from(path).ok()) {
            if event.kind.is_remove() {
                let unloaded = self.unload_removed_path(&path);
                if !unloaded.is_empty() {
                    files.extend(unloaded);
                    continue;
                }
            }

            let metadata = fs::metadata(&path).ok();
            let file_type = metadata.as_ref().map(|meta| meta.file_type());
            let is_file = file_type.as_ref().is_some_and(|it| it.is_file());
            let is_dir = file_type.as_ref().is_some_and(|it| it.is_dir());

            if is_dir && self.is_watched_dir(&path) {
                files.extend(self.load_created_directory(&path));
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

        files
    }

    fn is_watched_dir(&self, path: &AbsPathBuf) -> bool {
        self.watched_dirs.iter().any(|dir| dir.contains_dir(path))
    }

    fn is_watched_file(&self, path: &AbsPathBuf) -> bool {
        self.watched_files.contains(path)
            || self.watched_dirs.iter().any(|dir| dir.contains_file(path))
    }

    fn load_created_directory(&mut self, path: &AbsPathBuf) -> Vec<(AbsPathBuf, LoadResult)> {
        let dirs =
            self.watched_dirs.iter().filter(|dir| dir.contains_dir(path)).cloned().collect_vec();
        let mut files = Vec::new();

        for dir in dirs {
            let (watch_tx, watch_rx) = unbounded();
            files.extend(Self::load_directory_subtree(&watch_tx, &dir, path, true));
            drop(watch_tx);
            for path in watch_rx {
                self.watch(&path);
            }
        }

        files
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
    ) -> Vec<(AbsPathBuf, LoadResult)> {
        match entry {
            loader::Entry::Files(files) => files
                .into_iter()
                .map(|file| {
                    if watch && watch_tx.send(file.to_owned()).is_err() {
                        tracing::debug!("watched file path dropped because receiver is closed");
                    }
                    let contents = read(file.as_path());
                    (file, contents)
                })
                .collect_vec(),
            loader::Entry::Directories(dirs) => {
                let mut res = Vec::new();

                for root in dirs.include_roots() {
                    res.extend(Self::load_directory_subtree(watch_tx, &dirs, root, watch));
                }
                res
            }
        }
    }

    fn load_directory_subtree(
        watch_tx: &Sender<AbsPathBuf>,
        dirs: &loader::Directories,
        root: &AbsPathBuf,
        watch: bool,
    ) -> Vec<(AbsPathBuf, LoadResult)> {
        let walkdir = WalkDir::new(root).follow_links(true).into_iter().filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let Ok(path) = AbsPathBuf::try_from(entry.path().to_path_buf()) else {
                return false;
            };
            root == &path || dirs.contains_dir(&path)
        });

        let files = walkdir.filter_map(|it| it.ok()).filter_map(|entry| {
            let is_dir = entry.file_type().is_dir();
            let is_file = entry.file_type().is_file();
            let abs_path = AbsPathBuf::try_from(entry.into_path()).ok()?;

            if is_dir && watch && watch_tx.send(abs_path.to_owned()).is_err() {
                tracing::debug!("watched directory path dropped because receiver is closed");
            }

            if !is_file {
                return None;
            }

            if !dirs.contains_file(&abs_path) {
                return None;
            }

            Some(abs_path)
        });

        files
            .map(|file| {
                let contents = read(file.as_path());
                (file, contents)
            })
            .collect_vec()
    }

    fn watch(&mut self, path: &AbsPathBuf) {
        if let Some((watcher, _)) = &mut self.watcher {
            log_notify_error(watcher.watch(path.as_ref(), RecursiveMode::NonRecursive));
        }
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

fn log_notify_error<T>(res: notify::Result<T>) -> Option<T> {
    res.map_err(|err| tracing::warn!("notify error: {}", err)).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use notify::{
        Event as NotifyEvent, EventKind,
        event::{CreateKind, RemoveKind},
    };
    use utils::paths::AbsPathBuf;
    use vfs::{
        PathMatcher,
        loader::{self, Handle as _},
    };

    use super::*;

    struct TestDir {
        _dir: tempfile::TempDir,
        path: AbsPathBuf,
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
                | loader::Message::Progress { config_version, .. }
                    if *config_version == version =>
                {
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
        let dir = TestDir::new("vfs-notify-unload-file");
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
        let dir = TestDir::new("vfs-notify-missing-config-file");
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
        let dir = TestDir::new("vfs-notify-unload-directory");
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
        let dir = TestDir::new("vfs-notify-created-directory-load");
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

        let files = actor.process_notify_event(
            NotifyEvent::new(EventKind::Create(CreateKind::Folder))
                .add_path(path_buf(&created_dir)),
        );
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
        let dir = TestDir::new("vfs-notify-removed-directory-unload");
        let root = dir.join("workspace");
        let removed_dir = root.join("removed");
        let top = removed_dir.join("top.sv");
        let child = removed_dir.join("nested/child.sv");
        let sibling = root.join("sibling.sv");
        let mut actor = actor();
        actor.watched_dirs.push(watched_sv_dir(root));
        actor.loaded_paths.extend([top.clone(), child.clone(), sibling.clone()]);

        let mut files = actor.process_notify_event(
            NotifyEvent::new(EventKind::Remove(RemoveKind::Folder))
                .add_path(path_buf(&removed_dir)),
        );
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
