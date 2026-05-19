use crate::paths::{AbsPath, AbsPathBuf};

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

/// Returns lookup keys for the raw path and, when available, its filesystem
/// canonical form.
pub fn path_alias_keys(path: &AbsPath) -> Vec<PathKey> {
    let mut keys = vec![PathKey::from_abs_path(path)];

    if let Some(canonical) = std::fs::canonicalize(path)
        .ok()
        .and_then(|path| AbsPathBuf::try_from(path).ok())
        .map(|path| PathKey::from_abs_path(path.as_path()))
        && !keys.contains(&canonical)
    {
        keys.push(canonical);
    }

    keys
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
    fn path_alias_keys_include_raw_path() {
        let cwd = AbsPathBuf::assert_utf8(std::env::current_dir().unwrap());

        assert!(path_alias_keys(cwd.as_path()).contains(&PathKey::from_abs_path(cwd.as_path())));
    }
}
