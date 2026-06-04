#[cfg(test)]
mod architecture_tests;
pub mod directive_index;
mod macro_db;
pub mod trace;

pub use directive_index::{
    DirectiveEvent, DirectiveKind, DirectiveParam, DirectiveToken, MacroConditional,
    MacroConditionalKind, MacroDefine, MacroDirective, MacroDirectiveKind, MacroInclude,
    MacroIncludeTarget, MacroParam, MacroToken, MacroUndef, MacroUsage, PreprocFileIndex,
    literal_include_directives_from_index, preproc_file_index_from_directives,
};
pub use macro_db::{
    FileMacroInput, IncludeTargetAtResult, LiteralIncludeInput, MacroDb, MacroDbInput, MacroDefId,
    MacroDefinitionAtResult, MacroName, MacroPredefine, MacroProfileId, MacroQueryFailure,
    MacroReference, MacroReferencesResult, MacroSource, MacroUse, MacroUseId, MacroUseResolution,
    PredefineSource, SourceOrigin,
};
pub use trace::{
    CapabilityUnavailable, ConditionalEvaluation, ConditionalEvent, ConditionalEventId,
    ConditionalKind, ConditionalToken, ExpandedToken, ExpandedTokenId, ExpandedTokenOrigin,
    ExpansionId, FilePreprocTrace, FileRange, IncludeDirective, IncludeEvent, IncludeEventId,
    IncludeTarget, MacroArgument, MacroBody, MacroCall, MacroExpansionEvent,
    PREPROC_TRACE_CAPABILITY, PreprocFrame, PreprocFrameId, PreprocTrace, PreprocTraceResult,
    ProvenanceUnavailable, SourceInstance, SourceInstanceId, SourceProvenance, TraceCapability,
    TraceUnavailableReason, VirtualSource,
};
