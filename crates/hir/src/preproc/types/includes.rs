use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDirective {
    pub id: IncludeDirectiveId,
    pub file_id: FileId,
    pub include_index: usize,
    pub range: TextRange,
    pub target: IncludeTarget,
    pub status: IncludeDirectiveStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveBranch {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeTarget {
    Literal { path: SmolStr, resolved_file: Option<FileId> },
    Token { raw: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeDirectiveStatus {
    Resolved { file_id: FileId },
    Unresolved,
    Unavailable(SourcePreprocUnavailable),
}
