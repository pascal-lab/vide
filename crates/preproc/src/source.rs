mod model;
mod tables;
mod trace;
mod types;

/// Everything below is crate-internal; the `pub use` list above is the only
/// public API surface.
pub(crate) use tables::*;
/// Symbols consumed by `preproc-expand` and its tests. Kept explicit so the
/// compiler's dead-code analysis sees everything else; adding a new public
/// symbol here is a deliberate API decision, not a glob side effect.
pub use tables::{
    SourceIncludeDirective, SourceIncludeDirectiveId, SourceIncludeStatus, SourceMacroArgument,
    SourceMacroCall, SourceMacroCallId, SourceMacroDefinition, SourceMacroDefinitionId,
    SourceMacroReference, SourceMacroReferenceId, SourceMacroReferenceSite, SourceMacroResolution,
    SourcePreprocUnavailable,
};
pub(crate) use types::*;
pub use types::{
    MacroIncludeTarget, PreprocSourceId, SourceMacroParam, SourcePosition, SourcePreprocError,
    SourcePreprocModel, SourceRange,
};
