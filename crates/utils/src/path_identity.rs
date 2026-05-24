use std::path::Path;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::paths::{AbsPath, AbsPathBuf};

/// Normalized path spelling key used before filesystem identity is available.
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

/// Returns proven path spellings for a path crossing a process or FFI boundary.
///
/// This is intentionally not a total "canonical path" function. The raw path
/// key is always registered first, because it is the only identity we can
/// preserve without doing IO. The filesystem canonical path is added only when
/// the OS can prove one for the current path. Canonicalization goes through
/// `dunce` so Windows extended-length paths are converted back to ordinary path
/// spelling where possible. When canonicalization fails, for example because
/// the file does not exist yet or the filesystem rejects the lookup, we do not
/// invent another spelling.
///
/// These strings are safe to hand to external parsers as alternate names for
/// the same VFS text. File identity comparisons use [`FileIdentityKey`].
pub fn path_alias_paths(path: &AbsPath) -> Vec<AbsPathBuf> {
    let mut paths = vec![path.to_path_buf()];

    if let Some(canonical) = canonical_path(path)
        && !paths.contains(&canonical)
    {
        paths.push(canonical);
    }

    paths
}

pub fn path_alias_keys(path: &AbsPath) -> Vec<PathKey> {
    path_alias_paths(path).iter().map(|path| PathKey::from_abs_path(path)).collect()
}

/// Value identity for an existing filesystem object.
///
/// Unlike `same_file::Handle`, this key does not keep the file open after it is
/// computed.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct FileIdentityKey(FileIdentityKeyRepr);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum FileIdentityKeyRepr {
    #[cfg(unix)]
    Unix { dev: u64, ino: u64 },
    #[cfg(windows)]
    Windows { volume: u64, index: u64 },
}

impl FileIdentityKey {
    /// Returns a stable value identity for an existing filesystem path.
    ///
    /// The path may be opened or statted while computing the key, but the key
    /// itself does not retain any OS file handle.
    pub fn from_path(path: &AbsPath) -> Option<Self> {
        platform_file_identity_key(path.as_ref())
    }
}

/// Maps filesystem identity evidence to a caller-owned value.
///
/// Raw and canonical path aliases cover stable path spellings. OS identity keys
/// cover aliases that only the filesystem can prove, such as links. Callers
/// should insert a path again when a formerly missing file is created, because
/// identity evidence may become available later.
pub struct PathIdentityIndex<T> {
    aliases: FxHashMap<PathKey, T>,
    identities: FxHashMap<FileIdentityKey, T>,
}

impl<T> Default for PathIdentityIndex<T> {
    fn default() -> Self {
        Self { aliases: FxHashMap::default(), identities: FxHashMap::default() }
    }
}

impl<T: Copy> PathIdentityIndex<T> {
    /// Registers every path spelling and OS file identity that can be proven.
    ///
    /// Later inserts for the same alias replace earlier values. This mirrors
    /// the previous `PathKey -> FileId` map behavior and keeps collisions
    /// visible to the caller's insertion order instead of guessing which
    /// spelling is more correct.
    pub fn insert_path(&mut self, path: &AbsPath, value: T) {
        for key in path_alias_keys(path) {
            self.aliases.insert(key, value);
        }
        self.insert_identity(path, value);
    }

    pub fn get(&self, path: impl AsRef<str>) -> Option<T> {
        let path = path.as_ref();
        self.aliases.get(&PathKey::new(path)).copied().or_else(|| self.get_path(Path::new(path)))
    }

    pub fn get_path(&self, path: impl AsRef<Path>) -> Option<T> {
        let path = path.as_ref();
        if let Some(path) = path.to_str()
            && let Some(value) = self.aliases.get(&PathKey::new(path)).copied()
        {
            return Some(value);
        }

        if let Some(canonical) = canonical_path(path)
            && let Some(value) =
                self.aliases.get(&PathKey::from_abs_path(canonical.as_path())).copied()
        {
            return Some(value);
        }

        let identity = platform_file_identity_key(path)?;
        self.identities.get(&identity).copied()
    }

    fn insert_identity(&mut self, path: &AbsPath, value: T) {
        if let Some(identity) = FileIdentityKey::from_path(path) {
            self.identities.insert(identity, value);
        }
    }
}

/// Deduplicates paths by the same evidence model as [`PathIdentityIndex`].
#[derive(Default)]
pub struct PathIdentitySet {
    aliases: FxHashSet<PathKey>,
    identities: FxHashSet<FileIdentityKey>,
}

impl PathIdentitySet {
    /// Inserts all known aliases and returns whether none of them had been
    /// seen.
    pub fn insert_path(&mut self, path: &AbsPath) -> bool {
        let keys = path_alias_keys(path);
        let identity = FileIdentityKey::from_path(path);
        let is_new = keys.iter().all(|key| !self.aliases.contains(key))
            && identity.as_ref().is_none_or(|identity| !self.identities.contains(identity));

        self.aliases.extend(keys);
        if let Some(identity) = identity {
            self.identities.insert(identity);
        }

        is_new
    }
}

fn canonical_path(path: impl AsRef<Path>) -> Option<AbsPathBuf> {
    // `dunce` wraps `std::fs::canonicalize` but smooths over Windows
    // extended-length path spelling. It is still only an optional, OS-proven
    // spelling; file identity checks use a value key derived from metadata.
    dunce::canonicalize(path).ok().and_then(|path| AbsPathBuf::try_from(path).ok())
}

#[cfg(unix)]
fn platform_file_identity_key(path: &Path) -> Option<FileIdentityKey> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path).ok()?;
    Some(FileIdentityKey(FileIdentityKeyRepr::Unix { dev: metadata.dev(), ino: metadata.ino() }))
}

#[cfg(windows)]
fn platform_file_identity_key(path: &Path) -> Option<FileIdentityKey> {
    let handle = winapi_util::Handle::from_path_any(path).ok()?;
    let info = winapi_util::file::information(&handle).ok()?;
    Some(FileIdentityKey(FileIdentityKeyRepr::Windows {
        volume: info.volume_serial_number(),
        index: info.file_index(),
    }))
}

#[cfg(not(any(unix, windows)))]
fn platform_file_identity_key(_path: &Path) -> Option<FileIdentityKey> {
    None
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
    fn path_alias_paths_include_raw_path() {
        let cwd = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());

        assert!(path_alias_paths(cwd.as_path()).contains(&cwd));
    }

    #[test]
    fn path_alias_paths_do_not_invent_canonical_path_for_missing_path() {
        let dir = crate::test_support::TestDir::new("missing-path-alias");
        let missing = dir.join("missing.sv");
        let missing_path: &std::path::Path = missing.as_ref();

        assert!(!missing_path.exists());
        assert_eq!(path_alias_paths(missing.as_path()), vec![missing]);
    }

    #[test]
    fn path_identity_index_resolves_raw_path() {
        let cwd = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());
        let mut index = PathIdentityIndex::default();

        index.insert_path(cwd.as_path(), 1);

        assert_eq!(index.get(cwd.to_string()), Some(1));
    }

    #[test]
    fn path_identity_index_resolves_existing_path_by_file_identity() {
        let dir = crate::test_support::TestDir::new("file-identity");
        let path = dir.write("source.sv", "module top; endmodule\n");
        let alias = dir.join("alias.sv");
        let mut index = PathIdentityIndex::default();

        index.insert_path(path.as_path(), 1);

        std::fs::hard_link(&path, &alias).unwrap();

        assert_eq!(index.get_path(alias.as_path()), Some(1));
    }

    #[test]
    fn path_identity_set_detects_duplicate_raw_path() {
        let cwd = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());
        let mut set = PathIdentitySet::default();

        assert!(set.insert_path(cwd.as_path()));
        assert!(!set.insert_path(cwd.as_path()));
    }
}
