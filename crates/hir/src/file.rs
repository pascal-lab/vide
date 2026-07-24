use vfs::FileId;

use crate::{
    db::HirDb,
    hir_def::macro_file::{MacroFileId, macro_file_expansion},
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum HirFileId {
    File(FileId),
    Macro(MacroFileId),
}

impl HirFileId {
    pub fn as_file(self) -> Option<FileId> {
        match self {
            Self::File(file_id) => Some(file_id),
            Self::Macro(_) => None,
        }
    }

    pub fn expect_file(self) -> FileId {
        self.as_file().expect("HirFileId is not a source file")
    }

    /// The user-facing source file backing this HIR file: the file itself for
    /// real files, or the file containing the macro invocation for macro
    /// expansions. Returns `None` when a macro expansion's call site cannot be
    /// resolved.
    pub fn source_file_id(self, db: &dyn HirDb) -> Option<FileId> {
        match self {
            Self::File(file_id) => Some(file_id),
            Self::Macro(macro_file) => {
                macro_file_expansion(db, macro_file).map(|expansion| expansion.call_file_id)
            }
        }
    }
}

impl From<FileId> for HirFileId {
    fn from(file_id: FileId) -> HirFileId {
        HirFileId::File(file_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_file_returns_source_file_when_hir_file_is_real_file() {
        let file_id = FileId::from_raw(7);

        let hir_file_id = HirFileId::from(file_id);

        assert_eq!(hir_file_id.as_file(), Some(file_id));
        assert_eq!(hir_file_id.expect_file(), file_id);
    }
}
