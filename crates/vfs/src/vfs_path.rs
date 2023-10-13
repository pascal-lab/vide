// Abstract-ish representation of paths for VFS.
use std::fmt;

use utils::paths::{AbsPath, AbsPathBuf, RelPath};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct VfsPath(VfsPathKinds);

impl VfsPath {
    pub fn new_virtual_path(path: String) -> VfsPath {
        assert!(path.starts_with('/'));
        VfsPath(VfsPathKinds::VirtualPath(VirtualPath(path)))
    }

    pub fn new_real_path(path: String) -> VfsPath {
        VfsPath::from(AbsPathBuf::assert(path.into()))
    }

    /// Returns the `AbsPath` representation of `self` if `self` is on the file system.
    pub fn as_path(&self) -> Option<&AbsPath> {
        match &self.0 {
            VfsPathKinds::RealPath(it) => Some(it.as_path()),
            VfsPathKinds::VirtualPath(_) => None,
        }
    }

    /// Creates a new `VfsPath` with `path` adjoined to `self`.
    pub fn join(&self, path: &str) -> Option<VfsPath> {
        match &self.0 {
            VfsPathKinds::RealPath(it) => {
                let res = it.join(path).normalize();
                Some(VfsPath(VfsPathKinds::RealPath(res)))
            }
            VfsPathKinds::VirtualPath(it) => {
                let res = it.join(path)?;
                Some(VfsPath(VfsPathKinds::VirtualPath(res)))
            }
        }
    }

    /// Remove the last component of `self` if there is one.
    ///
    /// If `self` has no component, returns `false`; else returns `true`.
    ///
    /// # Example
    ///
    /// ```
    /// # use vfs::{AbsPathBuf, VfsPath};
    /// let mut path = VfsPath::from(AbsPathBuf::assert("/foo/bar".into()));
    /// assert!(path.pop());
    /// assert_eq!(path, VfsPath::from(AbsPathBuf::assert("/foo".into())));
    /// assert!(path.pop());
    /// assert_eq!(path, VfsPath::from(AbsPathBuf::assert("/".into())));
    /// assert!(!path.pop());
    /// ```
    pub fn pop(&mut self) -> bool {
        match &mut self.0 {
            VfsPathKinds::RealPath(it) => it.pop(),
            VfsPathKinds::VirtualPath(it) => it.pop(),
        }
    }

    /// Returns `true` if `other` is a prefix of `self`.
    pub fn starts_with(&self, other: &VfsPath) -> bool {
        match (&self.0, &other.0) {
            (VfsPathKinds::RealPath(lhs), VfsPathKinds::RealPath(rhs)) => lhs.starts_with(rhs),
            (VfsPathKinds::VirtualPath(lhs), VfsPathKinds::VirtualPath(rhs)) => lhs.starts_with(rhs),
            (VfsPathKinds::RealPath(_) | VfsPathKinds::VirtualPath(_), _) => false,
        }
    }

    pub fn strip_prefix(&self, other: &VfsPath) -> Option<&RelPath> {
        match (&self.0, &other.0) {
            (VfsPathKinds::RealPath(lhs), VfsPathKinds::RealPath(rhs)) => lhs.strip_prefix(rhs),
            (VfsPathKinds::VirtualPath(lhs), VfsPathKinds::VirtualPath(rhs)) => lhs.strip_prefix(rhs),
            (VfsPathKinds::RealPath(_) | VfsPathKinds::VirtualPath(_), _) => None,
        }
    }

    /// Returns the `VfsPath` without its final component, if there is one.
    ///
    /// Returns [`None`] if the path is a root or prefix.
    pub fn parent(&self) -> Option<VfsPath> {
        let mut parent = self.clone();
        if parent.pop() {
            Some(parent)
        } else {
            None
        }
    }

    /// Returns `self`'s base name and file extension.
    pub fn name_and_extension(&self) -> Option<(&str, Option<&str>)> {
        match &self.0 {
            VfsPathKinds::RealPath(p) => p.name_and_extension(),
            VfsPathKinds::VirtualPath(p) => p.name_and_extension(),
        }
    }

    /// **Don't make this `pub`**
    ///
    /// Encode the path in the given buffer.
    ///
    /// The encoding will be `0` if [`AbsPathBuf`], `1` if [`VirtualPath`], followed
    /// by `self`'s representation.
    ///
    /// Note that this encoding is dependent on the operating system.
    pub(crate) fn encode(&self, buf: &mut Vec<u8>) {
        let tag = match &self.0 {
            VfsPathKinds::RealPath(_) => 0,
            VfsPathKinds::VirtualPath(_) => 1,
        };
        buf.push(tag);
        match &self.0 {
            VfsPathKinds::RealPath(path) => {
                #[cfg(windows)]
                {
                    use windows_paths::Encode;
                    let path: &std::path::Path = path.as_ref();
                    let components = path.components();
                    let mut add_sep = false;
                    for component in components {
                        if add_sep {
                            windows_paths::SEP.encode(buf);
                        }
                        let len_before = buf.len();
                        match component {
                            std::path::Component::Prefix(prefix) => {
                                // kind() returns a normalized and comparable path prefix.
                                prefix.kind().encode(buf);
                            }
                            std::path::Component::RootDir => {
                                if !add_sep {
                                    component.as_os_str().encode(buf);
                                }
                            }
                            _ => component.as_os_str().encode(buf),
                        }

                        // some components may be encoded empty
                        add_sep = len_before != buf.len();
                    }
                }
                #[cfg(unix)]
                {
                    use std::os::unix::ffi::OsStrExt;
                    buf.extend(path.as_os_str().as_bytes());
                }
                #[cfg(not(any(windows, unix)))]
                {
                    buf.extend(path.as_os_str().to_string_lossy().as_bytes());
                }
            }
            VfsPathKinds::VirtualPath(VirtualPath(s)) => buf.extend(s.as_bytes()),
        }
    }
}

#[cfg(windows)]
mod windows_paths {
    pub(crate) trait Encode {
        fn encode(&self, buf: &mut Vec<u8>);
    }

    impl Encode for std::ffi::OsStr {
        fn encode(&self, buf: &mut Vec<u8>) {
            use std::os::windows::ffi::OsStrExt;
            for wchar in self.encode_wide() {
                buf.extend(wchar.to_le_bytes().iter().copied());
            }
        }
    }

    impl Encode for u8 {
        fn encode(&self, buf: &mut Vec<u8>) {
            let wide = *self as u16;
            buf.extend(wide.to_le_bytes().iter().copied())
        }
    }

    impl Encode for &str {
        fn encode(&self, buf: &mut Vec<u8>) {
            debug_assert!(self.is_ascii());
            for b in self.as_bytes() {
                b.encode(buf)
            }
        }
    }

    pub(crate) const SEP: &str = "\\";
    const VERBATIM: &str = "\\\\?\\";
    const UNC: &str = "UNC";
    const DEVICE: &str = "\\\\.\\";
    const COLON: &str = ":";

    impl Encode for std::path::Prefix<'_> {
        fn encode(&self, buf: &mut Vec<u8>) {
            match self {
                std::path::Prefix::Verbatim(c) => {
                    VERBATIM.encode(buf);
                    c.encode(buf);
                }
                std::path::Prefix::VerbatimUNC(server, share) => {
                    VERBATIM.encode(buf);
                    UNC.encode(buf);
                    SEP.encode(buf);
                    server.encode(buf);
                    SEP.encode(buf);
                    share.encode(buf);
                }
                std::path::Prefix::VerbatimDisk(d) => {
                    VERBATIM.encode(buf);
                    d.encode(buf);
                    COLON.encode(buf);
                }
                std::path::Prefix::DeviceNS(device) => {
                    DEVICE.encode(buf);
                    device.encode(buf);
                }
                std::path::Prefix::UNC(server, share) => {
                    SEP.encode(buf);
                    SEP.encode(buf);
                    server.encode(buf);
                    SEP.encode(buf);
                    share.encode(buf);
                }
                std::path::Prefix::Disk(d) => {
                    d.encode(buf);
                    COLON.encode(buf);
                }
            }
        }
    }
    #[test]
    fn paths_encoding() {
        // drive letter casing agnostic
        test_eq("C:/x.rs", "c:/x.rs");
        // separator agnostic
        test_eq("C:/x/y.rs", "C:\\x\\y.rs");

        fn test_eq(a: &str, b: &str) {
            let mut b1 = Vec::new();
            let mut b2 = Vec::new();
            vfs(a).encode(&mut b1);
            vfs(b).encode(&mut b2);
            assert_eq!(b1, b2);
        }
    }

    #[test]
    fn test_sep_root_dir_encoding() {
        let mut buf = Vec::new();
        vfs("C:/x/y").encode(&mut buf);
        assert_eq!(&buf, &[0, 67, 0, 58, 0, 92, 0, 120, 0, 92, 0, 121, 0])
    }

    #[cfg(test)]
    fn vfs(str: &str) -> super::VfsPath {
        use super::{AbsPathBuf, VfsPath};
        VfsPath::from(AbsPathBuf::try_from(str).unwrap())
    }
}

/// Internal, private representation of [`VfsPath`].
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
enum VfsPathKinds {
    RealPath(AbsPathBuf),
    VirtualPath(VirtualPath),
}

impl From<AbsPathBuf> for VfsPath {
    fn from(v: AbsPathBuf) -> Self {
        VfsPath(VfsPathKinds::RealPath(v.normalize()))
    }
}

impl fmt::Display for VfsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            VfsPathKinds::RealPath(it) => it.fmt(f),
            VfsPathKinds::VirtualPath(VirtualPath(it)) => it.fmt(f),
        }
    }
}

impl fmt::Debug for VfsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::Debug for VfsPathKinds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            VfsPathKinds::RealPath(it) => it.fmt(f),
            VfsPathKinds::VirtualPath(VirtualPath(it)) => it.fmt(f),
        }
    }
}

/// `/`-separated virtual path.
///
/// This is used to describe files that do not reside on the file system.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
struct VirtualPath(String);

impl VirtualPath {
    fn starts_with(&self, other: &VirtualPath) -> bool {
        self.0.starts_with(&other.0)
    }

    fn strip_prefix(&self, base: &VirtualPath) -> Option<&RelPath> {
        <_ as AsRef<std::path::Path>>::as_ref(&self.0)
            .strip_prefix(&base.0)
            .ok()
            .map(RelPath::new_unchecked)
    }

    fn pop(&mut self) -> bool {
        let pos = match self.0.rfind('/') {
            Some(pos) => pos,
            None => return false,
        };
        self.0 = self.0[..pos].to_string();
        true
    }

    fn join(&self, mut path: &str) -> Option<VirtualPath> {
        let mut res = self.clone();
        while path.starts_with("../") {
            if !res.pop() {
                return None;
            }
            path = &path["../".len()..];
        }
        path = path.trim_start_matches("./");
        res.0 = format!("{}/{path}", res.0);
        Some(res)
    }

    /// Returns `self`'s base name and file extension.
    ///
    /// # Returns
    /// - `None` if `self` ends with `"//"`.
    /// - `Some((name, None))` if `self`'s base contains no `.`, or only one `.` at
    /// the start.
    /// - `Some((name, Some(extension))` else.
    ///
    /// # Note
    /// The extension will not contains `.`. This means `"/foo/bar.baz.rs"` will
    /// return `Some(("bar.baz", Some("rs"))`.
    fn name_and_extension(&self) -> Option<(&str, Option<&str>)> {
        let file_path = if self.0.ends_with('/') { &self.0[..&self.0.len() - 1] } else { &self.0 };
        let file_name = match file_path.rfind('/') {
            Some(position) => &file_path[position + 1..],
            None => file_path,
        };

        if file_name.is_empty() {
            None
        } else {
            let mut file_stem_and_extension = file_name.rsplitn(2, '.');
            let extension = file_stem_and_extension.next();
            let file_stem = file_stem_and_extension.next();

            match (file_stem, extension) {
                (None, None) => None,
                (None | Some(""), Some(_)) => Some((file_name, None)),
                (Some(file_stem), extension) => Some((file_stem, extension)),
            }
        }
    }
}
