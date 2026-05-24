// Object safe interface for file watching and reading.
use std::fmt;

use utils::{
    lines::LineEnding,
    paths::{AbsPath, AbsPathBuf},
};

use crate::{PathGlobMatcher, PathMatcher};

/// File extensions loaded from recursive directory entries.
///
/// Exact file entries are represented by [`Entry::Files`] and do not rely on
/// extension expansion.
pub const SOURCE_FILE_EXTENSIONS: &[&str] = &["v", "sv", "vh", "svh", "svi", "map"];

/// A loader input boundary.
///
/// Exact files are loaded as listed. Directory entries are expanded using
/// extension and matcher rules and are also eligible for recursive watching.
#[derive(Debug, Clone)]
pub enum Entry {
    Files(Vec<AbsPathBuf>),
    Directories(Directories),
}

/// Recursive directory load policy.
#[derive(Debug, Clone, Default)]
pub struct Directories {
    pub extensions: Vec<String>,
    pub include: Vec<PathMatcher>,
    pub exclude: Vec<AbsPathBuf>,
    pub exclude_globs: Option<PathGlobMatcher>,
}

/// Complete loader configuration for one generation.
#[derive(Debug)]
pub struct Config {
    pub version: u32,
    pub to_load: Vec<Entry>,
    pub to_watch: Vec<usize>,
}

/// Messages sent by a loader generation back to the main loop.
pub enum Message {
    Progress { n_total: usize, n_done: usize, config_version: u32 },
    Loaded { files: Vec<(AbsPathBuf, LoadResult)>, config_version: u32 },
}

pub type Sender = crossbeam_channel::Sender<Message>;

/// Result of reading one file from the loader.
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
        self.include.iter().flat_map(PathMatcher::scan_roots)
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
            || self.exclude_globs.as_ref().is_some_and(|exclude| exclude.is_match(path))
    }
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Loaded { files, config_version } => f
                .debug_struct("Loaded")
                .field("n_files", &files.len())
                .field("config_version", config_version)
                .finish(),
            Message::Progress { n_total, n_done, config_version } => f
                .debug_struct("Progress")
                .field("n_total", n_total)
                .field("n_done", n_done)
                .field("config_version", config_version)
                .finish(),
        }
    }
}
