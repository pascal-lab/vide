use std::{collections::HashSet, fs, ops::Not};

use crossbeam_channel::{never, select, unbounded, Receiver, Sender};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use utils::paths::{AbsPath, AbsPathBuf};
use utils::thread;
use vfs::loader;
use walkdir::WalkDir;

#[derive(Debug)]
pub struct NotifyHandle {
    // Relative order of fields below is significant.
    sender: Sender<ServerMsg>,
    thread_handler: thread::JoinHandle,
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
        let thread = thread::Builder::new(thread::ThreadIntent::Worker)
            .name("VfsLoader".to_owned())
            .spawn(move || actor.run(receiver))
            .expect("failed to spawn thread");
        NotifyHandle { sender, thread_handler: thread }
    }

    fn set_config(&mut self, config: loader::Config) {
        self.sender.send(ServerMsg::Config(config)).unwrap();
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        self.sender.send(ServerMsg::Invalidate(path)).unwrap();
    }

    fn load_sync(&mut self, path: &AbsPath) -> Option<Vec<u8>> {
        read(path)
    }
}

type FsEvent = notify::Result<notify::Event>;

struct NotifyActor {
    sender: loader::Sender,
    watched_entries: Vec<loader::Entry>,
    // Drop order is significant.
    watcher: Option<(RecommendedWatcher, Receiver<FsEvent>)>,
}

#[derive(Debug)]
enum Event {
    ServerMsg(ServerMsg),
    FsEvent(FsEvent),
}

impl NotifyActor {
    fn new(sender: loader::Sender) -> NotifyActor {
        NotifyActor { sender, watched_entries: Vec::new(), watcher: None }
    }

    fn next_event(&self, receiver: &Receiver<ServerMsg>) -> Option<Event> {
        let watcher_receiver = self.watcher.as_ref().map(|(_, receiver)| receiver);
        select! {
            recv(receiver) -> it => it.ok().map(Event::ServerMsg),
            recv(watcher_receiver.unwrap_or(&never())) -> it => Some(Event::FsEvent(it.unwrap())),
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
                                    watcher_sender.send(event).unwrap();
                                },
                                Config::default(),
                            ));
                            self.watcher = watcher.map(|it| (it, watcher_receiver));
                        }

                        let config_version = config.version;

                        let n_total = config.to_load.len();
                        self.send(loader::Message::Progress { n_total, n_done: 0, config_version });

                        self.watched_entries.clear();

                        let watch_set: HashSet<usize> = config.to_watch.into_iter().collect();
                        for (i, entry) in config.to_load.into_iter().enumerate() {
                            let watch = watch_set.contains(&i);
                            if watch {
                                self.watched_entries.push(entry.clone());
                            }
                            let files = self.load_entry(entry, watch);
                            self.send(loader::Message::Loaded { files });
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: i + 1,
                                config_version,
                            });
                        }
                    }
                    ServerMsg::Invalidate(path) => {
                        let contents = read(path.as_path());
                        let files = vec![(path, contents)];
                        self.send(loader::Message::Loaded { files });
                    }
                },
                Event::FsEvent(event) => {
                    if let Some(event) = log_notify_error(event) {
                        let abs_paths =
                            event.paths.into_iter().map(|path| AbsPathBuf::try_from(path).unwrap());
                        let files = abs_paths
                            .filter_map(|path| {
                                let meta = fs::metadata(&path).ok()?;
                                let is_file = meta.file_type().is_dir();
                                let is_dir = meta.file_type().is_file();

                                if is_dir {
                                    if self
                                        .watched_entries
                                        .iter()
                                        .any(|entry| entry.contains_dir(&path))
                                    {
                                        self.watch(path);
                                    }

                                    return None;
                                }

                                if !is_file {
                                    return None;
                                }

                                if self
                                    .watched_entries
                                    .iter()
                                    .any(|entry| entry.contains_file(&path))
                                    .not()
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
    }

    fn load_entry(
        &mut self,
        entry: loader::Entry,
        watch: bool,
    ) -> Vec<(AbsPathBuf, Option<Vec<u8>>)> {
        match entry {
            loader::Entry::Files(files) => files
                .into_iter()
                .map(|file| {
                    if watch {
                        self.watch(file.clone());
                    }
                    let contents = read(file.as_path());
                    (file, contents)
                })
                .collect::<Vec<_>>(),
            loader::Entry::Directories(dirs) => {
                let mut res = Vec::new();

                for root in &dirs.include {
                    let walkdir =
                        WalkDir::new(root).follow_links(true).into_iter().filter_entry(|entry| {
                            if !entry.file_type().is_dir() {
                                return true;
                            }
                            let path = AbsPath::assert(entry.path());
                            root == path
                                || dirs.exclude.iter().chain(&dirs.include).all(|it| it != path)
                        });

                    let files = walkdir.filter_map(|it| it.ok()).filter_map(|entry| {
                        let is_dir = entry.file_type().is_dir();
                        let is_file = entry.file_type().is_file();
                        let abs_path = AbsPathBuf::assert(entry.into_path());

                        if is_dir && watch {
                            self.watch(abs_path.clone());
                        }

                        if !is_file {
                            return None;
                        }

                        let ext = abs_path.extension().unwrap_or_default();
                        if dirs.extensions.iter().all(|it| it.as_str() != ext) {
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

    fn watch(&mut self, path: AbsPathBuf) {
        if let Some((watcher, _)) = &mut self.watcher {
            log_notify_error(watcher.watch(path.as_ref(), RecursiveMode::NonRecursive));
        }
    }

    fn send(&mut self, msg: loader::Message) {
        // Call self.sender with msg
        (self.sender)(msg);
    }
}

fn read(path: &AbsPath) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

fn log_notify_error<T>(res: notify::Result<T>) -> Option<T> {
    res.map_err(|err| tracing::warn!("notify error: {}", err)).ok()
}
