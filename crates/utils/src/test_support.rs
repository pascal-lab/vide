use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::paths::{AbsPath, AbsPathBuf, Utf8Path};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct TestDir {
    path: AbsPathBuf,
}

impl TestDir {
    pub fn new(name: &str) -> Self {
        let path = unique_temp_path(name);
        fs::create_dir(&path).unwrap_or_else(|err| {
            panic!("failed to create test directory {path}: {err}");
        });
        Self { path }
    }

    pub fn path(&self) -> &AbsPath {
        self.path.as_path()
    }

    pub fn join(&self, path: impl AsRef<Utf8Path>) -> AbsPathBuf {
        self.path.absolutize(path)
    }

    pub fn create_dir_all(&self, path: impl AsRef<Utf8Path>) -> AbsPathBuf {
        let path = self.join(path);
        fs::create_dir_all(&path).unwrap_or_else(|err| {
            panic!("failed to create test directory {path}: {err}");
        });
        path
    }

    pub fn write(&self, path: impl AsRef<Utf8Path>, contents: impl AsRef<[u8]>) -> AbsPathBuf {
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

fn unique_temp_path(name: &str) -> AbsPathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("vizsla-{name}-{}-{stamp}-{id}", std::process::id()));
    AbsPathBuf::assert_utf8(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_removes_test_directory() {
        let path = {
            let dir = TestDir::new("test-support-drop");
            let file = dir.write("nested/file.sv", "module top; endmodule\n");
            let file: &std::path::Path = file.as_ref();

            assert!(file.exists());
            std::path::PathBuf::from(dir.path.to_path_buf())
        };

        assert!(!path.exists());
    }
}
