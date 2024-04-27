use la_arena::Idx;
use utils::text_edit::{TextRange, TextSize};
use vfs::vfs::FileId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FilePosition {
    pub file_id: FileId,
    pub offset: TextSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct FileRange {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug)]
pub struct RangeInfo<T> {
    pub range: TextRange,
    pub info: T,
}

pub type ErasedFileAstId = Idx<syntax::SyntaxNodePtr>;
