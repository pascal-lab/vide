use smol_str::SmolStr;
use vfs::{FileId, VfsPath};

use crate::ids::MacroExpansionId;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceDomain {
    RealFile { file_id: FileId },
    VirtualFile { file_id: FileId, path: VfsPath, origin: VirtualOrigin },
    VirtualDisplay { path: VfsPath, origin: VirtualOrigin },
    SlangSourceBuffer { buffer_id: u32 },
    ExpansionDisplay { expansion: MacroExpansionId },
    ExpansionParseBuffer { expansion: MacroExpansionId },
    Builtin { name: SmolStr },
    Unmapped { reason: SourceUnavailable },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VirtualOrigin {
    Predefines { profile: Option<u32> },
    Builtin { name: SmolStr },
    ExternalIncludeBuffer { source: u32 },
    Expansion { expansion: MacroExpansionId },
    Speculative { universe: u32 },
    Other { description: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceUnavailable {
    MissingSource { source: u32 },
    UnmappedSource { source: u32 },
    DisplayOnly,
    MacroCallAuthorityUnavailable,
    ExpansionAuthorityUnavailable,
    TokenProvenanceAuthorityUnavailable,
    Unsupported,
}
