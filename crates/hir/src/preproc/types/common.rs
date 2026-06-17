use super::*;

pub type PreprocResult<T> = Result<T, PreprocError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocError {
    SourceQuery(SourcePreprocQueryError),
    MissingRootSource,
    UnmappedSource {
        buffer_id: u32,
    },
    MismatchedDefinitionRangeFiles {
        event_id: u32,
        directive_file_id: FileId,
        name_file_id: FileId,
    },
    MismatchedReferenceRangeFiles {
        event_id: u32,
        directive_file_id: FileId,
        name_file_id: FileId,
    },
    MissingDefinitionNameRange {
        event_id: u32,
    },
    SourceMap(PreprocSourceMapError),
    SourceModel(SourcePreprocUnavailable),
    AmbiguousMacroReferenceContexts {
        contexts: usize,
    },
    AmbiguousMacroExpansionContexts {
        contexts: usize,
    },
    AmbiguousMacroParamContexts {
        contexts: usize,
    },
    AmbiguousMacroDefinitionContexts {
        contexts: usize,
    },
    AmbiguousIncludeTargets {
        targets: usize,
    },
    PartialPreprocContextIndex {
        skipped_models: usize,
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
