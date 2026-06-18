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
    VirtualDisplay { path: VfsPath, origin: PreprocVirtualOrigin },
    Unmapped(SourcePreprocUnavailable),
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
    MissingEmittedToken {
        token: SourceEmittedTokenId,
    },
    DisplayOnlyVirtualSource {
        path: VfsPath,
        origin: PreprocVirtualOrigin,
    },
}

mod mapping;
