// Object safe interface for file watching and reading.
use std::fmt;

use utils::{
    lines::LineEnding,
    paths::{AbsPath, AbsPathBuf},
};

use crate::PathSelection;

#[derive(Debug, Clone)]
pub enum Entry {
    Files(Vec<AbsPathBuf>),
    Directories(Directories),
}

#[derive(Debug, Clone, Default)]
pub struct Directories {
    pub extensions: Vec<String>,
    pub include: Vec<PathSelection>,
    pub exclude: Vec<AbsPathBuf>,
}

#[derive(Debug)]
pub struct Config {
    pub version: u32,
    pub to_load: Vec<Entry>,
    pub to_watch: Vec<usize>,
}

pub enum Message {
    Progress { n_total: usize, n_done: usize, config_version: u32 },
    Loaded { files: Vec<(AbsPathBuf, LoadResult)> },
}

pub type Sender = crossbeam_channel::Sender<Message>;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LoadResult {
    Loaded(String, LineEnding),
    LoadError,
    DecodeError,
}

pub trait Handle: fmt::Debug {
    fn spawn(sender: Sender) -> Self
    where
        Self: Sized;

    fn set_config(&mut self, config: Config);

    fn invalidate(&mut self, path: AbsPathBuf);

    fn load_sync(&mut self, path: &AbsPath) -> LoadResult;
}

impl Entry {
    pub fn contains_file(&self, path: &AbsPath) -> bool {
        match self {
            Entry::Files(files) => files.iter().any(|it| it == path),
            Entry::Directories(dirs) => dirs.contains_file(path),
        }
    }

    pub fn contains_dir(&self, path: &AbsPath) -> bool {
        match self {
            Entry::Files(_) => false,
            Entry::Directories(dirs) => dirs.contains_dir(path),
        }
    }
}

impl Directories {
    pub fn contains_file(&self, path: &AbsPath) -> bool {
        let ext = path.extension().unwrap_or_default();
        if self.extensions.iter().all(|it| it.as_str() != ext) {
            return false;
        }

        self.includes_file(path)
    }

    pub fn contains_dir(&self, path: &AbsPath) -> bool {
        self.includes_dir(path)
    }

    pub fn include_roots(&self) -> impl Iterator<Item = &AbsPathBuf> {
        self.include.iter().flat_map(PathSelection::roots)
    }

    /// Returns `true` if `path` is included in `self`.
    ///
    /// It is included if
    ///   - An include root is a prefix of `path`.
    ///   - No literal exclude prefix matches `path`.
    fn includes_file(&self, path: &AbsPath) -> bool {
        self.include.iter().any(|include| include.contains_file(path)) && !self.is_excluded(path)
    }

    fn includes_dir(&self, path: &AbsPath) -> bool {
        self.include.iter().any(|include| include.contains_dir(path))
            && !self.exclude.iter().any(|excl| path.starts_with(excl))
    }

    fn is_excluded(&self, path: &AbsPath) -> bool {
        self.exclude.iter().any(|excl| path.starts_with(excl))
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Loaded { files } => {
                f.debug_struct("Loaded").field("n_files", &files.len()).finish()
            }
            Message::Progress { n_total, n_done, config_version } => f
                .debug_struct("Progress")
                .field("n_total", n_total)
                .field("n_done", n_done)
                .field("config_version", config_version)
                .finish(),
        }
    }
}
