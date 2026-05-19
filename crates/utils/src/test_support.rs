use std::fs;

use crate::paths::{AbsPath, AbsPathBuf, Utf8Path};

#[derive(Debug)]
pub struct TestDir {
    _temp_dir: tempfile::TempDir,
    path: AbsPathBuf,
}

impl TestDir {
    pub fn new(name: &str) -> Self {
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("vizsla-{name}-"))
            .tempdir()
            .unwrap_or_else(|err| {
                panic!("failed to create test directory for {name}: {err}");
            });
        let path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
        Self { _temp_dir: temp_dir, path }
    }

    pub fn new_in(parent: impl AsRef<std::path::Path>, name: &str) -> Self {
        let temp_dir = tempfile::Builder::new()
            .prefix(&format!("vizsla-{name}-"))
            .tempdir_in(parent)
            .unwrap_or_else(|err| {
                panic!("failed to create test directory for {name}: {err}");
            });
        let path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
        Self { _temp_dir: temp_dir, path }
    }

    pub fn from_temp_dir(temp_dir: tempfile::TempDir) -> Self {
        let path = AbsPathBuf::assert_utf8(temp_dir.path().to_path_buf());
        Self { _temp_dir: temp_dir, path }
    }

    pub fn into_temp_dir(self) -> tempfile::TempDir {
        self._temp_dir
    }

    pub fn into_path(self) -> AbsPathBuf {
        let path = self.path.clone();
        let _ = self._temp_dir.keep();
        path
    }

    pub fn close(self) {
        let path = self.path.to_string();
        self._temp_dir.close().unwrap_or_else(|err| {
            panic!("failed to remove test directory {path}: {err}");
        });
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
