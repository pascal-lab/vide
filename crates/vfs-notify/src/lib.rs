use std::{fs, sync::atomic::AtomicUsize};

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
}

#[derive(Debug)]
enum ServerMsg {
    Config(loader::Config),
    Invalidate(AbsPathBuf),
}

impl loader::Handle for NotifyHandle {
    fn spawn(sender: loader::Sender) -> NotifyHandle {
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
        NotifyHandle { sender, _handler: thread }
    }

    fn set_config(&mut self, config: loader::Config) {
        if self.sender.send(ServerMsg::Config(config)).is_err() {
            tracing::error!("failed to send VFS config to loader thread");
        }
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        if self.sender.send(ServerMsg::Invalidate(path)).is_err() {
            tracing::error!("failed to send VFS invalidation to loader thread");
        }
    }

    fn load_sync(&mut self, path: &AbsPath) -> LoadResult {
        read(path)
    }
}

type NotifyEvent = notify::Result<notify::Event>;

struct NotifyActor {
    sender: loader::Sender,
    watched_files: FxHashSet<AbsPathBuf>,
    watched_dirs: Vec<loader::Directories>,
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
            watched_files: FxHashSet::default(),
            watched_dirs: Vec::new(),
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
                        let n_total = config.to_load.len();
                        self.send(loader::Message::Progress { n_total, n_done: 0, config_version });

                        self.watched_files.clear();
                        self.watched_dirs.clear();

                        let (entry_tx, entry_rx) = unbounded();
                        let (watch_tx, watch_rx) = unbounded();
                        let processed = AtomicUsize::new(0);

                        config.to_load.into_par_iter().enumerate().for_each(|(i, entry)| {
                            let do_watch = config.to_watch.contains(&i);
                            if do_watch && entry_tx.send(entry.clone()).is_err() {
                                tracing::debug!("watched entry dropped because receiver is closed");
                            }
                            let files = Self::load_entry(&watch_tx, entry, do_watch);
                            self.send(loader::Message::Loaded { files });
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: 1 + processed
                                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel),
                                config_version,
                            });
                        });

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
                    }
                    ServerMsg::Invalidate(path) => {
                        let contents = read(path.as_path());
                        let files = vec![(path, contents)];
                        self.send(loader::Message::Loaded { files });
                    }
                },
                Event::NotifyEvent(event) => {
                    let Some(event) = log_notify_error(event) else {
                        continue;
                    };

                    if !(event.kind.is_create() || event.kind.is_modify() || event.kind.is_remove())
                    {
                        continue;
                    }

                    let files = event
                        .paths
                        .into_iter()
                        .filter_map(|path| AbsPathBuf::try_from(path).ok())
                        .filter_map(|path| {
                            let meta = fs::metadata(&path).ok()?;
                            let file_type = meta.file_type();
                            let is_file = file_type.is_file();
                            let is_dir = file_type.is_dir();

                            if is_dir && self.watched_dirs.iter().any(|dir| dir.contains_dir(&path))
                            {
                                self.watch(&path);
                                return None;
                            }

                            if !is_file {
                                return None;
                            }

                            if !(self.watched_files.contains(&path)
                                || self.watched_dirs.iter().any(|dir| dir.contains_file(&path)))
                            {
                                return None;
                            }

                            let contents = read(&path);

                            Some((path, contents))
                        })
                        .collect();

                    self.send(loader::Message::Loaded { files });
                }
            }
        }
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
                    let walkdir =
                        WalkDir::new(root).follow_links(true).into_iter().filter_entry(|entry| {
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
                            tracing::debug!(
                                "watched directory path dropped because receiver is closed"
                            );
                        }

                        if !is_file {
                            return None;
                        }

                        if !dirs.contains_file(&abs_path) {
                            return None;
                        }

                        Some(abs_path)
                    });

                    res.extend(files.map(|file| {
                        let contents = read(file.as_path());
                        (file, contents)
                    }));
                }
                res
            }
        }
    }

    fn watch(&mut self, path: &AbsPathBuf) {
        if let Some((watcher, _)) = &mut self.watcher {
            log_notify_error(watcher.watch(path.as_ref(), RecursiveMode::NonRecursive));
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
