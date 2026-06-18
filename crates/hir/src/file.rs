use vfs::FileId;

use crate::hir_def::macro_file::MacroFileId;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
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

    pub fn file_id(self) -> FileId {
        self.expect_file()
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
        let file_id = FileId(7);

        let hir_file_id = HirFileId::from(file_id);

        assert_eq!(hir_file_id.as_file(), Some(file_id));
        assert_eq!(hir_file_id.file_id(), file_id);
    }
}
