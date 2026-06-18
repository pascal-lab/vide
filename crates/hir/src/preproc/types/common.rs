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
    /// Source preprocessor query failed (slang/preproc errors).
    SourceQuery(SourcePreprocQueryError),
    /// Mapping a preproc source range back to file/text-range failed.
    SourceMap(PreprocSourceMapError),
    /// The source-side preproc model marked the requested data unavailable.
    SourceModel(SourcePreprocUnavailable),
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
    /// The preproc context index was partial because some compilation models
    /// could not be queried; queries that ran were valid but the result is
    /// not authoritative across the whole project.
    PartialPreprocContextIndex { skipped_models: usize },
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
