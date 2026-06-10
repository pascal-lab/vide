use utils::line_index::{TextRange, TextSize};
use vfs::FileId;

use crate::ids::{SourceDomainId, SpanId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub domain: SourceDomainId,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceSelection {
    pub full: SpanId,
    pub focus: Option<SpanId>,
}

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
