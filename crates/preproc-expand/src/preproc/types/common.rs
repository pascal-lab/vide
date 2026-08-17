use super::*;

pub type PreprocResult<T> = Result<T, PreprocError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmbiguousKind {
    MacroReference,
    MacroExpansion,
    MacroParam,
    MacroDefinition,
    IncludeTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RangeFilesKind {
    Definition,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocError {
    /// Source preprocessor query or source-range mapping failed.
    SourceQuery(SourcePreprocQueryError),
    /// Multiple distinct preproc contexts produced conflicting answers and
    /// no single context can be selected.
    Ambiguous { kind: AmbiguousKind, count: usize },
    /// A mapping straddled two different files where a single file was
    /// required (definition or reference). `event_id` identifies the slang
    /// trace event for diagnosis.
    MismatchedRangeFiles {
        kind: RangeFilesKind,
        event_id: u32,
        directive_file_id: FileId,
        name_file_id: FileId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MacroDefinitionId {
    Source(SourceMacroDefinitionId),
    ConfiguredPredefine { file_id: FileId, range: TextRange },
}

impl From<SourceMacroDefinitionId> for MacroDefinitionId {
    fn from(value: SourceMacroDefinitionId) -> Self {
        Self::Source(value)
    }
}

impl From<SourcePreprocQueryError> for PreprocError {
    fn from(value: SourcePreprocQueryError) -> Self {
        Self::SourceQuery(value)
    }
}

impl From<SourcePreprocError> for PreprocError {
    fn from(value: SourcePreprocError) -> Self {
        Self::SourceQuery(SourcePreprocQueryError::Model(value))
    }
}
