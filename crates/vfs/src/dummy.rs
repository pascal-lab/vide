//! Loader with no OS file watching (e.g. wasm).

use std::fmt;

use crossbeam_channel::Sender;
use utils::paths::{AbsPath, AbsPathBuf};

use crate::loader::{self, LoadingProgress};

pub struct DummyHandle {
    sender: Sender<loader::Message>,
}

impl loader::Handle for DummyHandle {
    fn spawn(sender: loader::Sender) -> Self {
        Self { sender }
    }

    fn set_config(&mut self, config: loader::Config) {
        let config_version = config.version;
        let n_total = config.load.len();
        let _ = self.sender.send(loader::Message::Progress {
            n_total,
            n_done: LoadingProgress::Started,
            dir: None,
            config_version,
        });
        for _ in &config.load {
            let _ = self.sender.send(loader::Message::Loaded { files: Vec::new() });
        }
        let _ = self.sender.send(loader::Message::Progress {
            n_total,
            n_done: LoadingProgress::Finished,
            dir: None,
            config_version,
        });
    }

    fn invalidate(&mut self, path: AbsPathBuf) {
        let contents = std::fs::read(path.as_path()).ok();
        let _ = self.sender.send(loader::Message::Changed { files: vec![(path, contents)] });
    }

    fn load_sync(&mut self, path: &AbsPath) -> Option<Vec<u8>> {
        std::fs::read(path).ok()
    }
}

impl fmt::Debug for DummyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DummyHandle").finish_non_exhaustive()
    }
}
