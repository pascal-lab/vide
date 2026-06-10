use smol_str::SmolStr;
use vfs::FileId;

use crate::ids::{IncludeDirectiveId, SourceContextId};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceContext {
    CompilationRoot {
        profile_id: Option<u32>,
        root_file: FileId,
    },
    IncludeContext {
        parent: SourceContextId,
        include_directive: IncludeDirectiveId,
        included_file: FileId,
    },
    Speculative {
        base: SourceContextId,
        reason: SpeculativeReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpeculativeReason {
    Completion,
    CodeAction,
    UnsavedOverlay,
    MacroExpansionPreview,
    Other(SmolStr),
}
