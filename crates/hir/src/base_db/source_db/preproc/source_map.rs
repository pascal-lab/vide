use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocSourceMap {
    entries: FxHashMap<PreprocSourceId, PreprocSourceMapping>,
    predefine_sources: FxHashMap<PreprocSourceId, PreprocManifestSource>,
    text_lengths: FxHashMap<PreprocSourceId, usize>,
    range_offsets: FxHashMap<PreprocSourceId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapping {
    RealFile(FileId),
    VirtualFile { file_id: FileId, path: VfsPath, origin: PreprocVirtualOrigin },
    Unmapped(PreprocUnavailableReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocUnavailableReason {
    DetachedSource { buffer_id: u32 },
    MissingPredefineSourceText { buffer_id: u32 },
    UnverifiedPredefineSource { buffer_id: u32 },
    MissingMacroCall { call: SourceMacroCallId },
    MissingMacroExpansion { call: SourceMacroCallId },
    UnknownMacroUsageDefinition { definition: MacroDefinitionId },
}

impl From<SourcePreprocUnavailable> for PreprocUnavailableReason {
    fn from(reason: SourcePreprocUnavailable) -> Self {
        match reason {
            SourcePreprocUnavailable::DetachedSource { source } => {
                Self::DetachedSource { buffer_id: source.raw() }
            }
            SourcePreprocUnavailable::MissingPredefineSourceText { source } => {
                Self::MissingPredefineSourceText { buffer_id: source.raw() }
            }
            SourcePreprocUnavailable::UnverifiedPredefineSource { source } => {
                Self::UnverifiedPredefineSource { buffer_id: source.raw() }
            }
            SourcePreprocUnavailable::MissingMacroCall { call } => Self::MissingMacroCall { call },
            SourcePreprocUnavailable::MissingMacroExpansion { call } => {
                Self::MissingMacroExpansion { call }
            }
            SourcePreprocUnavailable::UnknownMacroUsageDefinition { definition } => {
                Self::UnknownMacroUsageDefinition { definition }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreprocManifestSource {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocVirtualOrigin {
    Predefines { profile: Option<CompilationProfileId> },
    Builtin { name: SmolStr },
    ExternalIncludeBuffer { buffer_id: u32 },
    Speculative { universe: PreprocSpeculativeUniverseId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreprocSpeculativeUniverseId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapError {
    MissingSource { buffer_id: u32 },
    UnmappedSource { buffer_id: u32, reason: PreprocUnavailableReason },
    RangeOutOfBounds { buffer_id: u32, range: TextRange, mapped_range: TextRange, text_len: usize },
    MissingEmittedToken { token: SourceEmittedTokenId },
}

mod mapping;
