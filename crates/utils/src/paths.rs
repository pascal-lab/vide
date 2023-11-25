// Thin wrappers around `std::path`, distinguishing between absolute and
// relative paths.

use std::{
    borrow::Borrow,
    convert::{TryFrom, TryInto},
    ffi::OsStr,
    fmt, ops,
    path::{Component, Path, PathBuf, Prefix},
};

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct AbsPathBuf(PathBuf);

impl From<AbsPathBuf> for PathBuf {
    fn from(AbsPathBuf(path_buf): AbsPathBuf) -> PathBuf {
        path_buf
    }
}

impl ops::Deref for AbsPathBuf {
    type Target = AbsPath;
    fn deref(&self) -> &AbsPath {
        self.as_path()
    }
}

impl AsRef<Path> for AbsPathBuf {
    fn as_ref(&self) -> &Path {
        self.0.as_path()
    }
}

impl AsRef<AbsPath> for AbsPathBuf {
    fn as_ref(&self) -> &AbsPath {
        self.as_path()
    }
}

impl Borrow<AbsPath> for AbsPathBuf {
    fn borrow(&self) -> &AbsPath {
        self.as_path()
    }
}

impl TryFrom<PathBuf> for AbsPathBuf {
    type Error = PathBuf;
    fn try_from(path_buf: PathBuf) -> Result<AbsPathBuf, PathBuf> {
        if !path_buf.is_absolute() {
            return Err(path_buf);
        }
        Ok(AbsPathBuf(path_buf))
    }
}

impl TryFrom<&str> for AbsPathBuf {
    type Error = PathBuf;
    fn try_from(path: &str) -> Result<AbsPathBuf, PathBuf> {
        AbsPathBuf::try_from(PathBuf::from(path))
    }
}

impl PartialEq<AbsPath> for AbsPathBuf {
    fn eq(&self, other: &AbsPath) -> bool {
        self.as_path() == other
    }
}

impl AbsPathBuf {
    pub fn assert(path: PathBuf) -> AbsPathBuf {
        AbsPathBuf::try_from(path)
            .unwrap_or_else(|path| panic!("expected absolute path, got {}", path.display()))
    }

    pub fn as_path(&self) -> &AbsPath {
        AbsPath::assert(self.0.as_path())
    }

    pub fn pop(&mut self) -> bool {
        self.0.pop()
    }
}

impl fmt::Display for AbsPathBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.display(), f)
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct AbsPath(Path);

impl AsRef<Path> for AbsPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl ToOwned for AbsPath {
    type Owned = AbsPathBuf;

    fn to_owned(&self) -> Self::Owned {
        AbsPathBuf(self.0.to_owned())
    }
}

impl<'a> TryFrom<&'a Path> for &'a AbsPath {
    type Error = &'a Path;
    fn try_from(path: &'a Path) -> Result<&'a AbsPath, &'a Path> {
        if !path.is_absolute() {
            return Err(path);
        }
        Ok(AbsPath::assert(path))
    }
}

impl AbsPath {
    pub fn assert(path: &Path) -> &AbsPath {
        assert!(path.is_absolute());
        unsafe { &*(path as *const Path as *const AbsPath) }
    }

    pub fn parent(&self) -> Option<&AbsPath> {
        self.0.parent().map(AbsPath::assert)
    }

    pub fn absolutize(&self, path: impl AsRef<Path>) -> AbsPathBuf {
        self.join(path).normalize()
    }

    pub fn join(&self, path: impl AsRef<Path>) -> AbsPathBuf {
        self.as_ref().join(path).try_into().unwrap()
    }

    pub fn normalize(&self) -> AbsPathBuf {
        AbsPathBuf(normalize(&self.0))
    }

    pub fn to_path_buf(&self) -> AbsPathBuf {
        AbsPathBuf::try_from(self.0.to_path_buf()).unwrap()
    }

    pub fn strip_prefix(&self, base: &AbsPath) -> Option<&RelPath> {
        self.0.strip_prefix(base).ok().map(RelPath::new_unchecked)
    }
    pub fn starts_with(&self, base: &AbsPath) -> bool {
        self.0.starts_with(&base.0)
    }
    pub fn ends_with(&self, suffix: &RelPath) -> bool {
        self.0.ends_with(&suffix.0)
    }

    pub fn name_and_extension(&self) -> Option<(&str, Option<&str>)> {
        Some((
            self.file_stem()?.to_str()?,
            self.extension().and_then(|extension| extension.to_str()),
        ))
    }

    pub fn file_name(&self) -> Option<&OsStr> {
        self.0.file_name()
    }
    pub fn extension(&self) -> Option<&OsStr> {
        self.0.extension()
    }
    pub fn file_stem(&self) -> Option<&OsStr> {
        self.0.file_stem()
    }
    pub fn as_os_str(&self) -> &OsStr {
        self.0.as_os_str()
    }
}

impl fmt::Display for AbsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.display(), f)
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct RelPathBuf(PathBuf);

impl From<RelPathBuf> for PathBuf {
    fn from(RelPathBuf(path_buf): RelPathBuf) -> PathBuf {
        path_buf
    }
}

impl ops::Deref for RelPathBuf {
    type Target = RelPath;
    fn deref(&self) -> &RelPath {
        self.as_path()
    }
}

impl AsRef<Path> for RelPathBuf {
    fn as_ref(&self) -> &Path {
        self.0.as_path()
    }
}

impl TryFrom<PathBuf> for RelPathBuf {
    type Error = PathBuf;
    fn try_from(path_buf: PathBuf) -> Result<RelPathBuf, PathBuf> {
        if !path_buf.is_relative() {
            return Err(path_buf);
        }
        Ok(RelPathBuf(path_buf))
    }
}

impl TryFrom<&str> for RelPathBuf {
    type Error = PathBuf;
    fn try_from(path: &str) -> Result<RelPathBuf, PathBuf> {
        RelPathBuf::try_from(PathBuf::from(path))
    }
}

impl RelPathBuf {
    pub fn as_path(&self) -> &RelPath {
        RelPath::new_unchecked(self.0.as_path())
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct RelPath(Path);

impl AsRef<Path> for RelPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl RelPath {
    pub fn new_unchecked(path: &Path) -> &RelPath {
        unsafe { &*(path as *const Path as *const RelPath) }
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().copied() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                ret.pop();
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

pub fn patch_path_prefix(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        // VSCode might report paths with the file drive in lowercase, but this can mess
        // So we just uppercase the drive letter here unconditionally.
        let mut comps = path.components();
        match comps.next() {
            Some(Component::Prefix(prefix)) => {
                let prefix = match prefix.kind() {
                    Prefix::Disk(d) => {
                        format!("{}:", d.to_ascii_uppercase() as char)
                    }
                    Prefix::VerbatimDisk(d) => {
                        format!(r"\\?\{}:", d.to_ascii_uppercase() as char)
                    }
                    _ => return path,
                };
                let mut path = PathBuf::new();
                path.push(prefix);
                path.extend(comps);
                path
            }
            _ => path,
        }
    } else {
        path
    }
}

pub fn sort_and_remove_subfolders(paths: &mut Vec<AbsPathBuf>) {
    paths.sort();
    paths.dedup_by(|a, b| a.starts_with(b));
}

// test for sort_and_remove_subfolders
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_and_remove_subfolders() {
        let mut paths: Vec<AbsPathBuf> = vec![
            "/home/username/Documents/b",
            "/home/username/Downloads/a/b",
            "/home/username/Documents/a",
            "/home/username/Downloads/a",
            "/home/username/Downloads",
        ]
        .into_iter()
        .map(PathBuf::from)
        .map(AbsPathBuf::assert)
        .collect::<Vec<_>>();
        sort_and_remove_subfolders(&mut paths);
        assert_eq!(
            paths,
            vec![
                AbsPathBuf::assert(PathBuf::from("/home/username/Documents/a")),
                AbsPathBuf::assert(PathBuf::from("/home/username/Documents/b")),
                AbsPathBuf::assert(PathBuf::from("/home/username/Downloads")),
            ]
        );
    }
}
