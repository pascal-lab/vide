// Object safe interface for file watching and reading.
use std::fmt;

use utils::paths::{AbsPath, AbsPathBuf};

#[derive(Debug, Clone)]
pub enum Entry {
    Files(Vec<AbsPathBuf>),
    Directories(Directories),
}

#[derive(Debug, Clone, Default)]
pub struct Directories {
    pub extensions: Vec<String>,
    pub include: Vec<AbsPathBuf>,
    pub exclude: Vec<AbsPathBuf>,
}

#[derive(Debug)]
pub struct Config {
    pub version: u32,
    pub load: Vec<Entry>,
    pub watch: Vec<usize>,
}

pub enum Message {
    Progress { n_total: usize, n_done: usize, config_version: u32 },
    Loaded { files: Vec<(AbsPathBuf, Option<Vec<u8>>)> },
}

pub type Sender = Box<dyn Fn(Message) + Send>;

pub trait Handle: fmt::Debug {
    fn spawn(sender: Sender) -> Self
    where
        Self: Sized;

    fn set_config(&mut self, config: Config);

    fn invalidate(&mut self, path: AbsPathBuf);

    fn load_sync(&mut self, path: &AbsPath) -> Option<Vec<u8>>;
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

        self.includes_path(path)
    }

    pub fn contains_dir(&self, path: &AbsPath) -> bool {
        self.includes_path(path)
    }

    /// Returns `true` if `path` is included in `self`.
    ///
    /// It is included if
    ///   - An element in `self.include` is a prefix of `path`.
    ///   - This path is longer than any element in `self.exclude` that is a prefix
    ///     of `path`. In case of equality, exclusion wins.
    fn includes_path(&self, path: &AbsPath) -> bool {
        let mut longest_incl: Option<&AbsPathBuf> = None;
        for incl in &self.include {
            if path.starts_with(incl) && longest_incl.map_or(true, |path| incl.starts_with(path)) {
                longest_incl = Some(incl);
            }
        }

        let longest_incl = match longest_incl {
            Some(it) => it,
            None => return false,
        };

        !self.exclude.iter()
                     .any(|excl| path.starts_with(excl) && excl.starts_with(longest_incl))
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
