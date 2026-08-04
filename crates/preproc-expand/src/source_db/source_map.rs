use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub mapping: PreprocSourceMapping,
    pub text_len: usize,
    pub range_offset: usize,
    pub manifest_source: Option<PreprocManifestSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocSourceMap {
    entries: FxHashMap<PreprocSourceId, SourceEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocSourceMapping {
    RealFile(FileId),
    /// `file_id: None` is a display-only virtual source (e.g. the predefine
    /// buffer) that has no user-facing file.
    VirtualFile {
        file_id: Option<FileId>,
        path: VfsPath,
        origin: PreprocVirtualOrigin,
    },
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
    ExternalIncludeBuffer { source: PreprocSourceId },
}

mod mapping;
