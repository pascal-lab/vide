use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use utils::paths::{AbsPathBuf, Utf8Path};

pub(crate) struct TestDir {
    path: AbsPathBuf,
}

impl TestDir {
    pub(crate) fn new(name: &str) -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("vizsla-vfs-{name}-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&path).unwrap_or_else(|err| {
            panic!("failed to create test directory {}: {err}", path.display());
        });
        let path = AbsPathBuf::assert_utf8(path);
        Self { path }
    }

    pub(crate) fn join(&self, path: impl AsRef<Utf8Path>) -> AbsPathBuf {
        self.path.absolutize(path)
    }

    pub(crate) fn write(
        &self,
        path: impl AsRef<Utf8Path>,
        contents: impl AsRef<[u8]>,
    ) -> AbsPathBuf {
        let path = self.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|err| {
                panic!("failed to create test directory {parent}: {err}");
            });
        }
        fs::write(&path, contents).unwrap_or_else(|err| {
            panic!("failed to write test file {path}: {err}");
        });
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
