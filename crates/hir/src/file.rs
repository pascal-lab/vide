//! Opaque handle for "which file or macro expansion a HIR item lives in".
//!
//! Wraps `preproc_expand::file::HirFileId` so IDE code never names the
//! preproc-expand type directly.

use preproc_expand::file::HirFileId;
use vfs::FileId;

/// A located file, either a real source file or a macro expansion.
///
/// The underlying identifier is intentionally hidden; IDE code obtains a `File`
/// from the facade and turns it back into a `FileId` via [`File::expect_file`]
/// or [`File::file_id_opt`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct File(HirFileId);

impl File {
    /// Returns the underlying real `FileId`.
    ///
    /// Panics if this `File` is a macro expansion rather than a source file.
    /// IDE entry points that only handle source files should call this;
    /// callers that must distinguish expansions should use
    /// [`File::file_id_opt`].
    pub fn expect_file(self) -> FileId {
        self.0.expect_file()
    }

    /// Returns `Some(FileId)` for real source files, `None` for macro
    /// expansions.
    pub fn file_id_opt(self) -> Option<FileId> {
        match self.0 {
            HirFileId::File(id) => Some(id),
            _ => None,
        }
    }

    /// Facade-internal access to the wrapped identifier. Only other modules of
    /// this crate call this; it is `pub(crate)` so IDE cannot reach the
    /// underlying `HirFileId`.
    pub(crate) fn into_inner(self) -> HirFileId {
        self.0
    }
}

impl From<FileId> for File {
    fn from(id: FileId) -> Self {
        File(HirFileId::File(id))
    }
}

impl From<HirFileId> for File {
    fn from(id: HirFileId) -> Self {
        File(id)
    }
}