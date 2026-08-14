//! `loader::Handle` backed by `walkdir` and OS `notify` (best-effort).

use std::{
    fs,
    path::{Component, Path},
    sync::atomic::AtomicUsize,
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender, select, unbounded};
use notify::{Config, EventKind, RecursiveMode, Watcher, event::AccessKind};
use rayon::iter::{IndexedParallelIterator as _, IntoParallelIterator as _, ParallelIterator};
use rustc_hash::FxHashSet;
use utils::paths::{AbsPath, AbsPathBuf, Utf8PathBuf};
use walkdir::WalkDir;

use crate::loader::{self, LoadingProgress};

// FSEvents watcher registration can block while starting or stopping its
// CFRunLoop, which must never hold workspace readiness hostage. PollWatcher has
// bounded setup and shutdown behavior and preserves recursive change detection
// on macOS.
#[cfg(target_os = "macos")]
type BackendWatcher = notify::PollWatcher;
#[cfg(not(target_os = "macos"))]
type BackendWatcher = notify::RecommendedWatcher;

#[cfg(target_os = "macos")]
const WATCHER_BACKEND: &str = "poll";
#[cfg(not(target_os = "macos"))]
const WATCHER_BACKEND: &str = "recommended";

#[derive(Debug)]
pub struct NotifyHandle {
    // Relative order of fields below is significant.
    sender: Sender<Message>,
    _thread: utils::thread::JoinHandle,
}

#[derive(Debug)]
enum Message {
    Config(loader::Config),
    Invalidate(AbsPathBuf),
}

impl loader::Handle for NotifyHandle {
    fn spawn(sender: loader::Sender) -> NotifyHandle {
        let actor = NotifyActor::new(sender);
        let (sender, receiver) = unbounded::<Message>();
        let thread = utils::thread::Builder::new(utils::thread::ThreadIntent::Worker)
            .name("VfsLoader".to_owned())
            .spawn(move || actor.run(receiver))
            .expect("failed to spawn thread");
        NotifyHandle { sender, _thread: thread }
    }

    fn set_config(&mut self, config: loader::Config) {
        self.sender.send(Message::Config(config)).unwrap();
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        self.sender.send(Message::Invalidate(path)).unwrap();
    }

    fn load_sync(&mut self, path: &AbsPath) -> Option<Vec<u8>> {
        read(path)
    }
}

type NotifyEvent = notify::Result<notify::Event>;

struct NotifyActor {
    sender: loader::Sender,
    watched_file_entries: FxHashSet<AbsPathBuf>,
    watched_dir_entries: Vec<loader::Directories>,
    seen_paths: FxHashSet<AbsPathBuf>,
    // Drop order is significant.
    watcher: Option<(BackendWatcher, Receiver<NotifyEvent>)>,
}

#[derive(Debug)]
enum Event {
    Message(Message),
    NotifyEvent(NotifyEvent),
}

impl NotifyActor {
    fn new(sender: loader::Sender) -> NotifyActor {
        NotifyActor {
            sender,
            watched_dir_entries: Vec::new(),
            watched_file_entries: FxHashSet::default(),
            seen_paths: FxHashSet::default(),
            watcher: None,
        }
    }

    fn next_event(&self, receiver: &Receiver<Message>) -> Option<Event> {
        let Some((_, watcher_receiver)) = &self.watcher else {
            return receiver.recv().ok().map(Event::Message);
        };

        select! {
            recv(receiver) -> it => it.ok().map(Event::Message),
            recv(watcher_receiver) -> it => Some(Event::NotifyEvent(it.unwrap())),
        }
    }

    fn run(mut self, inbox: Receiver<Message>) {
        while let Some(event) = self.next_event(&inbox) {
            tracing::debug!(?event, "vfs-notify event");
            match event {
                Event::Message(msg) => match msg {
                    Message::Config(config) => {
                        self.watcher = None;
                        if !config.watch.is_empty() {
                            let (watcher_sender, watcher_receiver) = unbounded();
                            let watcher_config =
                                Config::default().with_poll_interval(Duration::from_secs(1));
                            let watcher = BackendWatcher::new(
                                move |event| {
                                    // A disconnected receiver means the actor was dropped. Do not
                                    // panic in the platform callback because that only obscures the
                                    // shutdown cause.
                                    _ = watcher_sender.send(event);
                                },
                                watcher_config,
                            )
                            .map_err(|error| {
                                tracing::error!(
                                    %error,
                                    backend = WATCHER_BACKEND,
                                    "failed to create file watcher"
                                );
                            })
                            .ok();
                            self.watcher = watcher.map(|it| (it, watcher_receiver));
                        }

                        let config_version = config.version;

                        let n_total = config.load.len();
                        self.watched_dir_entries.clear();
                        self.watched_file_entries.clear();
                        self.seen_paths.clear();

                        self.send(loader::Message::Progress {
                            n_total,
                            n_done: LoadingProgress::Started,
                            config_version,
                            dir: None,
                        });

                        let (entry_tx, entry_rx) = unbounded();
                        let (watch_tx, watch_rx) = unbounded();
                        let processed = AtomicUsize::new(0);

                        config.load.into_par_iter().enumerate().for_each(|(i, entry)| {
                            let do_watch = config.watch.contains(&i);
                            if do_watch {
                                _ = entry_tx.send(entry.clone());
                            }
                            let files = Self::load_entry(
                                |f| _ = watch_tx.send(f.to_owned()),
                                entry,
                                do_watch,
                                |file| {
                                    self.send(loader::Message::Progress {
                                        n_total,
                                        n_done: LoadingProgress::Progress(
                                            processed.load(std::sync::atomic::Ordering::Relaxed),
                                        ),
                                        dir: Some(file),
                                        config_version,
                                    });
                                },
                            );
                            self.send(loader::Message::Loaded { files });
                            self.send(loader::Message::Progress {
                                n_total,
                                n_done: LoadingProgress::Progress(
                                    processed.fetch_add(1, std::sync::atomic::Ordering::AcqRel) + 1,
                                ),
                                config_version,
                                dir: None,
                            });
                        });

                        drop(watch_tx);
                        for path in watch_rx {
                            self.watch(&path);
                        }

                        drop(entry_tx);
                        for entry in entry_rx {
                            match entry {
                                loader::Entry::Files(files) => {
                                    self.watched_file_entries.extend(files)
                                }
                                loader::Entry::Directories(dir) => {
                                    self.watched_dir_entries.push(dir)
                                }
                            }
                        }

                        self.send(loader::Message::Progress {
                            n_total,
                            n_done: LoadingProgress::Finished,
                            config_version,
                            dir: None,
                        });
                    }
                    Message::Invalidate(path) => {
                        let contents = read(path.as_path());
                        let files = vec![(path, contents)];
                        self.send(loader::Message::Changed { files });
                    }
                },
                Event::NotifyEvent(event) => {
                    let event = match event {
                        Ok(event) => event,
                        Err(error) => {
                            tracing::error!(%error, backend = WATCHER_BACKEND, "file watcher error");
                            continue;
                        }
                    };
                    if let EventKind::Create(_)
                    | EventKind::Modify(_)
                    | EventKind::Remove(_)
                    | EventKind::Access(AccessKind::Open(_)) = event.kind
                    {
                        let abs_paths: Vec<AbsPathBuf> = event
                            .paths
                            .into_iter()
                            .filter_map(|path| {
                                Some(
                                    AbsPathBuf::try_from(Utf8PathBuf::from_path_buf(path).ok()?)
                                        .expect("path is absolute"),
                                )
                            })
                            .collect();

                        let mut saw_new_file = false;
                        for abs_path in &abs_paths {
                            if self.seen_paths.insert(abs_path.clone()) {
                                saw_new_file = true;
                            }
                        }

                        // Only consider access events for files that we haven't seen
                        // before.
                        //
                        // This is important on FUSE filesystems, where we may not get a
                        // Create event. In other cases we're about to access the file, so
                        // we don't want an infinite loop where processing an Access event
                        // creates another Access event.
                        if matches!(event.kind, EventKind::Access(_)) && !saw_new_file {
                            continue;
                        }

                        let files = abs_paths
                            .into_iter()
                            .filter_map(|path| -> Option<(AbsPathBuf, Option<Vec<u8>>)> {
                                // Ignore events for files/directories that we're not watching.
                                if !(self.watched_file_entries.contains(&path)
                                    || self
                                        .watched_dir_entries
                                        .iter()
                                        .any(|dir| dir.contains_file(&path)))
                                {
                                    return None;
                                }

                                // For removed files, fs::metadata() will return Err, but
                                // we still want to update the VFS.
                                if matches!(event.kind, EventKind::Remove(_)) {
                                    return Some((path, None));
                                }

                                let meta = fs::metadata(&path).ok()?;
                                if meta.file_type().is_dir()
                                    && self
                                        .watched_dir_entries
                                        .iter()
                                        .any(|dir| dir.contains_dir(&path))
                                {
                                    self.watch(path.as_ref());
                                    return None;
                                }

                                if !meta.file_type().is_file() {
                                    return None;
                                }

                                let contents = read(&path);
                                Some((path, contents))
                            })
                            .collect();
                        self.send(loader::Message::Changed { files });
                    }
                }
            }
        }
    }

    fn load_entry(
        mut watch: impl FnMut(&Path),
        entry: loader::Entry,
        do_watch: bool,
        send_message: impl Fn(AbsPathBuf),
    ) -> Vec<(AbsPathBuf, Option<Vec<u8>>)> {
        match entry {
            loader::Entry::Files(files) => files
                .into_iter()
                .map(|file| {
                    if do_watch {
                        watch(file.as_ref());
                    }
                    let contents = read(file.as_path());
                    (file, contents)
                })
                .collect::<Vec<_>>(),
            loader::Entry::Directories(dirs) => {
                let mut res = Vec::new();

                for root in &dirs.include {
                    send_message(root.clone());
                    let walkdir =
                        WalkDir::new(root).follow_links(true).into_iter().filter_entry(|entry| {
                            if !entry.file_type().is_dir() {
                                return true;
                            }
                            let path = entry.path();

                            if path_might_be_cyclic(path) {
                                return false;
                            }

                            // We want to filter out subdirectories that are roots themselves,
                            // because they will be visited separately.
                            let path: &Path = path;
                            dirs.exclude
                                .iter()
                                .all(|it| <AbsPathBuf as AsRef<Path>>::as_ref(it) != path)
                                && (<AbsPathBuf as AsRef<Path>>::as_ref(root) == path
                                    || dirs
                                        .include
                                        .iter()
                                        .all(|it| <AbsPathBuf as AsRef<Path>>::as_ref(it) != path))
                        });

                    let files = walkdir.filter_map(|it| it.ok()).filter_map(|entry| {
                        let depth = entry.depth();
                        let is_dir = entry.file_type().is_dir();
                        let is_file = entry.file_type().is_file();
                        let abs_path = AbsPathBuf::try_from(
                            Utf8PathBuf::from_path_buf(entry.into_path()).ok()?,
                        )
                        .ok()?;
                        if depth < 2 && is_dir {
                            send_message(abs_path.clone());
                        }
                        if is_dir && do_watch {
                            watch(abs_path.as_ref());
                        }
                        if !is_file {
                            return None;
                        }
                        if !dirs.contains_file(abs_path.as_path()) {
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

    fn watch(&mut self, path: &Path) {
        if let Some((watcher, _)) = &mut self.watcher
            && let Err(error) = watcher.watch(path, RecursiveMode::Recursive)
        {
            tracing::error!(
                %error,
                path = %path.display(),
                backend = WATCHER_BACKEND,
                "failed to register file watcher path"
            );
        }
    }

    #[track_caller]
    fn send(&self, msg: loader::Message) {
        self.sender.send(msg).unwrap();
    }
}

fn read(path: &AbsPath) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Is `path` a symlink to a parent directory?
///
/// Including this path is guaranteed to cause an infinite loop. This
/// heuristic is not sufficient to catch all symlink cycles (it's
/// possible to construct cycle using two or more symlinks), but it
/// catches common cases.
fn path_might_be_cyclic(path: &Path) -> bool {
    let Ok(destination) = std::fs::read_link(path) else {
        return false;
    };

    // If the symlink is of the form "../..", it's a parent symlink.
    let is_relative_parent =
        destination.components().all(|c| matches!(c, Component::CurDir | Component::ParentDir));

    is_relative_parent || path.starts_with(destination)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crossbeam_channel::unbounded;
    use tempfile::tempdir;
    use utils::paths::{AbsPathBuf, Utf8PathBuf};

    use super::NotifyHandle;
    use crate::loader::{self, Handle as _, LoadingProgress};

    #[test]
    fn watcher_setup_finishes_loading() {
        let temp_dir = tempdir().unwrap();
        std::fs::write(temp_dir.path().join("top.sv"), "module top; endmodule\n").unwrap();
        let root = AbsPathBuf::try_from(
            Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap(),
        )
        .unwrap();
        let directories = loader::Directories {
            extensions: vec!["sv".to_owned()],
            include: vec![root],
            exclude: Vec::new(),
        };

        let config = loader::Config {
            version: 7,
            load: vec![loader::Entry::Directories(directories)],
            watch: vec![0],
        };

        let (sender, receiver) = unbounded();
        let mut handle = NotifyHandle::spawn(sender);
        handle.set_config(config);

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut loaded_batches = 0;
        let mut journal = Vec::new();
        loop {
            let message = receiver.recv_deadline(deadline).unwrap_or_else(|error| {
                panic!(
                    "VFS configuration did not finish before the deadline: {error}; events: {journal:#?}"
                )
            });
            journal.push(format!("{message:?}"));
            match message {
                loader::Message::Loaded { .. } => loaded_batches += 1,
                loader::Message::Progress {
                    config_version: 7,
                    n_done: LoadingProgress::Finished,
                    ..
                } => break,
                _ => {}
            }
        }

        assert_eq!(loaded_batches, 1);

        // The previous macOS FSEvents watcher could wait indefinitely for its run loop
        // when this handle was dropped.
        drop(handle);
    }
}
