use std::path::Path;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::paths::AbsPath;

/// Normalized path spelling key for paths that cross process or FFI boundaries.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct PathKey(String);

impl PathKey {
    /// Stable key for paths that cross process or FFI boundaries.
    pub fn new(path: impl AsRef<str>) -> Self {
        Self(normalize_path_key(path.as_ref()))
    }

    pub fn from_abs_path(path: &AbsPath) -> Self {
        Self::new(path.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Maps path spellings to a caller-owned value.
///
/// A path identity is the spelling we handed out, not something the filesystem
/// is asked to prove: every path that crosses a boundary leaves through this
/// index, so it comes back as the same spelling. Lookups therefore never touch
/// the filesystem, which matters because the include search probes far more
/// paths than exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathIdentityIndex<T> {
    paths: FxHashMap<PathKey, T>,
}

impl<T> Default for PathIdentityIndex<T> {
    fn default() -> Self {
        Self { paths: FxHashMap::default() }
    }
}

impl<T: Copy> PathIdentityIndex<T> {
    /// Later inserts for the same spelling replace earlier values.
    pub fn insert_path(&mut self, path: &AbsPath, value: T) {
        self.paths.insert(PathKey::from_abs_path(path), value);
    }

    pub fn get(&self, path: impl AsRef<str>) -> Option<T> {
        self.paths.get(&PathKey::new(path.as_ref())).copied()
    }

    pub fn get_path(&self, path: impl AsRef<Path>) -> Option<T> {
        self.get(path.as_ref().to_str()?)
    }
}

/// Deduplicates paths by the same spelling identity as [`PathIdentityIndex`].
#[derive(Default)]
pub struct PathIdentitySet {
    paths: FxHashSet<PathKey>,
}

impl PathIdentitySet {
    /// Returns whether this spelling had not been seen.
    pub fn insert_path(&mut self, path: &AbsPath) -> bool {
        self.paths.insert(PathKey::from_abs_path(path))
    }
}

fn normalize_path_key(path: &str) -> String {
    let mut path = path.replace('\\', "/");

    if let Some(rest) = path.strip_prefix("//?/UNC/") {
        path = format!("//{rest}");
    } else if let Some(rest) = path.strip_prefix("//?/") {
        path = rest.to_owned();
    }

    if path.as_bytes().get(1) == Some(&b':') && path.as_bytes()[0].is_ascii_alphabetic() {
        let drive = path[0..1].to_ascii_uppercase();
        path.replace_range(0..1, &drive);
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AbsPathBuf;

    #[test]
    fn path_key_normalizes_separators() {
        assert_eq!(PathKey::new(r"C:\rtl\top.sv").as_str(), "C:/rtl/top.sv");
    }

    #[test]
    fn path_key_normalizes_windows_drive_letter() {
        assert_eq!(PathKey::new(r"c:\rtl\top.sv").as_str(), "C:/rtl/top.sv");
    }

    #[test]
    fn path_key_strips_windows_verbatim_prefixes() {
        assert_eq!(PathKey::new(r"\\?\c:\rtl\top.sv").as_str(), "C:/rtl/top.sv");
        assert_eq!(PathKey::new(r"\\?\UNC\server\share\top.sv").as_str(), "//server/share/top.sv");
    }

    #[test]
    fn path_identity_index_resolves_the_spelling_it_was_given() {
        let cwd = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());
        let mut index = PathIdentityIndex::default();

        index.insert_path(cwd.as_path(), 1);

        assert_eq!(index.get(cwd.to_string()), Some(1));
    }

    #[test]
    fn path_identity_index_keeps_parent_directory_segments() {
        let mut index = PathIdentityIndex::default();
        let path = if cfg!(windows) {
            AbsPathBuf::assert("C:\\repo\\rtl\\config.vh".into())
        } else {
            AbsPathBuf::assert("/repo/rtl/config.vh".into())
        };
        index.insert_path(path.as_path(), 1);

        let slang_path = if cfg!(windows) {
            r"C:\repo\rtl\..\rtl\config.vh"
        } else {
            "/repo/rtl/../rtl/config.vh"
        };
        assert_eq!(index.get(slang_path), None);
        assert_eq!(index.get(path.to_string()), Some(1));
    }

    #[test]
    fn path_identity_index_resolves_a_path_that_does_not_exist() {
        let dir = crate::test_support::TestDir::new("unwritten-path-identity");
        let missing = dir.join("missing.sv");
        let missing_path: &std::path::Path = missing.as_ref();
        let mut index = PathIdentityIndex::default();

        index.insert_path(missing.as_path(), 1);

        assert!(!missing_path.exists());
        assert_eq!(index.get_path(missing_path), Some(1));
    }

    #[test]
    fn path_identity_set_detects_duplicate_path() {
        let cwd = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());
        let mut set = PathIdentitySet::default();

        assert!(set.insert_path(cwd.as_path()));
        assert!(!set.insert_path(cwd.as_path()));
    }
}
