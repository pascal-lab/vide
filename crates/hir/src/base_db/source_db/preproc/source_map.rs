use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocSourceMap {
    entries: FxHashMap<PreprocSourceId, PreprocSourceMapping>,
    expansion_entries: FxHashMap<SourceMacroExpansionId, PreprocExpansionMapping>,
    predefine_sources: FxHashMap<PreprocSourceId, PreprocManifestSource>,
    text_lengths: FxHashMap<PreprocSourceId, usize>,
    range_offsets: FxHashMap<PreprocSourceId, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapping {
    RealFile(FileId),
    VirtualFile { file_id: FileId, path: VfsPath, origin: PreprocVirtualOrigin },
    VirtualDisplay { path: VfsPath, origin: PreprocVirtualOrigin },
    Unmapped(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocExpansionMapping {
    pub origin: PreprocVirtualOrigin,
    pub emitted_range: SourceEmittedTokenRange,
    pub display: PreprocExpansionDisplay,
    pub source_buffer: PreprocExpansionSourceBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocExpansionDisplay {
    pub path: VfsPath,
    pub text: String,
    token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocExpansionSourceBuffer {
    ParseStable {
        file_id: FileId,
        path: VfsPath,
        text: String,
        token_ranges: FxHashMap<SourceEmittedTokenId, TextRange>,
    },
    DisplayOnly {
        path: VfsPath,
    },
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
    ExternalIncludeBuffer { source: PreprocSourceId },
    Expansion { expansion: SourceMacroExpansionId },
    Speculative { universe: PreprocSpeculativeUniverseId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreprocSpeculativeUniverseId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapError {
    MissingSource {
        source: PreprocSourceId,
    },
    UnmappedSource {
        source: PreprocSourceId,
        reason: SourcePreprocUnavailable,
    },
    RangeOutOfBounds {
        source: PreprocSourceId,
        range: TextRange,
        mapped_range: TextRange,
        text_len: usize,
    },
    MissingExpansionVirtualFile {
        expansion: SourceMacroExpansionId,
    },
    MissingEmittedToken {
        token: SourceEmittedTokenId,
    },
    MissingEmittedTokenRange {
        range: SourceEmittedTokenRange,
    },
    DisplayOnlyVirtualSource {
        path: VfsPath,
        origin: PreprocVirtualOrigin,
    },
}

mod expansion;
mod mapping;
