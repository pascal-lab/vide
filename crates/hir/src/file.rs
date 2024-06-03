use vfs::vfs::FileId;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct HirFileId(pub FileId);

impl From<FileId> for HirFileId {
    fn from(file_id: FileId) -> HirFileId {
        HirFileId(file_id)
    }
}
