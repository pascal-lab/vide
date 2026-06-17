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
    Unavailable {
        reason: PreprocUnavailable,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocUnavailable {
    Source(SourcePreprocUnavailable),
    AmbiguousMacroReferenceContexts { contexts: usize },
    AmbiguousMacroExpansionContexts { contexts: usize },
    AmbiguousMacroParamContexts { contexts: usize },
    AmbiguousMacroDefinitionContexts { contexts: usize },
    AmbiguousDiagnosticProvenance { targets: usize },
    AmbiguousIncludeTargets { targets: usize },
    PartialPreprocContextIndex { skipped_models: usize },
    DisplayOnlyVirtualExpansion { path: VfsPath, origin: PreprocVirtualOrigin },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocAvailability {
    Complete,
    Partial,
    Unavailable(PreprocUnavailable),
}

macro_rules! mapped_preproc_id {
    ($name:ident, $core:ty) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name($core);

        impl $name {
            pub fn raw(self) -> usize {
                self.0.raw()
            }
        }

        impl From<$core> for $name {
            fn from(value: $core) -> Self {
                Self(value)
            }
        }
    };
}

mapped_preproc_id!(MacroReferenceId, SourceMacroReferenceId);
mapped_preproc_id!(IncludeDirectiveId, SourceIncludeDirectiveId);
mapped_preproc_id!(MacroCallId, SourceMacroCallId);
mapped_preproc_id!(MacroExpansionId, SourceMacroExpansionId);

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

pub(crate) const CONFIGURED_PREDEFINE_DEFINE_INDEX: usize = usize::MAX;
pub(crate) const CONFIGURED_PREDEFINE_EVENT_ID: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappedPreprocSource {
    RealFile { file_id: FileId },
    VirtualFile { file_id: FileId, path: vfs::VfsPath, origin: PreprocVirtualOrigin },
    VirtualDisplay { path: vfs::VfsPath, origin: PreprocVirtualOrigin },
}

impl MappedPreprocSource {
    pub fn file_id(&self) -> Option<FileId> {
        match self {
            Self::RealFile { file_id } | Self::VirtualFile { file_id, .. } => Some(*file_id),
            Self::VirtualDisplay { .. } => None,
        }
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
