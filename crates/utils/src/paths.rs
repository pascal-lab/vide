// Re-export rust-analyzer's typed path wrappers, plus a few Vide-specific
// path utilities that don't belong upstream.
//
// We delegate AbsPath/AbsPathBuf/RelPath/RelPathBuf and the camino re-exports
// to the `ra_ap_paths` crate (the published form of rust-analyzer's
// `crates/paths`), which is the exact code we previously vendored here. That
// crate fixes the `RelPath(Utf8Path)` layout (our vendored copy had
// `RelPath(Path)`, making `new_unchecked` a transmute UB) and adds methods like
// `AbsPath::as_str` that we were missing.
//
// The only additions below are:
//   * `abs_path_buf_from_path_buf` — fallible `PathBuf` → `AbsPathBuf` helper
//     used by VFS file walks and path canonicalization. The orphan rule blocks
//     a `TryFrom<PathBuf>` impl on the upstream type, so we expose a free
//     function instead.
//   * `patch_path_prefix` / `sort_and_remove_subfolders` — Vide-specific
//     helpers with no upstream equivalent.

use std::path::PathBuf;

pub use camino::{self, *};
pub use ra_ap_paths::{AbsPath, AbsPathBuf, RelPath, RelPathBuf};

/// Fallibly convert a `PathBuf` to an `AbsPathBuf`.
///
/// Returns `None` if `path` is not absolute or not valid UTF-8.
pub fn abs_path_buf_from_path_buf(path: PathBuf) -> Option<AbsPathBuf> {
    Utf8PathBuf::from_path_buf(path).ok().and_then(|p| AbsPathBuf::try_from(p).ok())
}

/// Fallibly convert a `PathBuf` to an `AbsPathBuf`, returning the original
/// `PathBuf` as the error on failure (not absolute, or not valid UTF-8).
///
/// This mirrors the `TryFrom<PathBuf>` impl we previously vendored, which the
/// orphan rule prevents us from providing for the upstream `AbsPathBuf` type.
pub fn try_abs_path_buf_from_path_buf(path: PathBuf) -> Result<AbsPathBuf, PathBuf> {
    match Utf8PathBuf::from_path_buf(path) {
        Ok(utf8) => AbsPathBuf::try_from(utf8).map_err(|utf8| utf8.into_std_path_buf()),
        Err(path_buf) => Err(path_buf),
    }
}

/// Uppercase the Windows drive letter, if any.
///
/// VSCode reports paths with a lowercase drive letter (e.g. `c:\`), which
/// breaks path comparisons against canonical forms that use `C:\`. On
/// non-Windows this is a no-op.
pub fn patch_path_prefix(path: PathBuf) -> PathBuf {
    if cfg!(windows) {
        use std::path::{Component, Prefix};

        let mut comps = path.components();
        match comps.next() {
            Some(Component::Prefix(prefix)) => {
                let prefix = match prefix.kind() {
                    Prefix::Disk(d) => format!("{}:", d.to_ascii_uppercase() as char),
                    Prefix::VerbatimDisk(d) => format!(r"\\?\{}:", d.to_ascii_uppercase() as char),
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

/// Sort `paths` and drop any entry that is a subfolder of another entry.
pub fn sort_and_remove_subfolders(paths: &mut Vec<AbsPathBuf>) {
    paths.sort();
    paths.dedup_by(|a, b| a.starts_with(b));
}
