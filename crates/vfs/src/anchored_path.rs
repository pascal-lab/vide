use crate::vfs::FileId;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AnchoredPathBuf {
    pub anchor_id: FileId,
    pub path: String,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct AnchoredPath<'a> {
    pub anchor_id: FileId,
    pub path: &'a str,
}
