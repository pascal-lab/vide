use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDirective {
    pub id: SourceIncludeDirectiveId,
    pub file_id: FileId,
    pub range: TextRange,
    pub target: IncludeTarget,
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
