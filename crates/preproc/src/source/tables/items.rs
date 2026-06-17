use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMacroReferenceSite {
    Usage { usage_index: usize },
    ConditionalToken { conditional_index: usize, token_index: usize },
    IncludeGuardIfNDef { conditional_index: usize, token_index: usize },
    MacroBodyToken { call: SourceMacroCallId, token_index: usize },
    ExpansionToken { emitted_token: SourceEmittedTokenId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDefinition {
    pub id: SourceMacroDefinitionId,
    pub event_id: SourcePreprocEventId,
    pub trace_definition: Option<MacroDefinitionId>,
    pub name: SmolStr,
    pub name_range: SourceRange,
    pub directive_range: SourceRange,
    pub params: Option<Vec<SourceMacroParam>>,
    pub body_tokens: Vec<SourceMacroToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroReference {
    pub id: SourceMacroReferenceId,
    pub event_id: SourcePreprocEventId,
    pub site: SourceMacroReferenceSite,
    pub name: SmolStr,
    pub name_range: SourceRange,
    pub directive_range: SourceRange,
    pub resolution: SourceMacroResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMacroResolution {
    Resolved {
        definition: SourceMacroDefinitionId,
        reason: SourceMacroResolutionReason,
        include_chain: Vec<SourceIncludeChainEntry>,
    },
    Undefined,
    Unavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMacroResolutionReason {
    VisibleDefinition,
    IncludeGuardIfNDef,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceIncludeGraph {
    pub(in crate::source::tables) directives: Vec<SourceIncludeDirective>,
    pub(in crate::source::tables) edges: Vec<SourceIncludeEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIncludeDirective {
    pub id: SourceIncludeDirectiveId,
    pub event_id: SourcePreprocEventId,
    pub directive_range: SourceRange,
    pub target: MacroIncludeTarget,
    pub target_range: Option<SourceRange>,
    pub resolved_source: Option<PreprocSourceId>,
    pub status: SourceIncludeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceIncludeStatus {
    Resolved { source: PreprocSourceId },
    Unresolved,
    Unavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroStateTimeline {
    pub(in crate::source::tables) states: Vec<SourceMacroState>,
    pub(in crate::source::tables) checkpoints: Vec<SourceMacroStateCheckpoint>,
    pub(in crate::source::tables) source_order_scopes:
        BTreeMap<PreprocSourceId, SourceMacroStateSourceScope>,
    pub(in crate::source::tables) source_order_boundaries:
        BTreeMap<PreprocSourceId, Vec<SourceMacroStatePositionBoundary>>,
    pub(in crate::source::tables) final_source_order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroState {
    pub id: SourceMacroStateId,
    pub definitions: BTreeMap<SmolStr, SourceMacroDefinitionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroStateCheckpoint {
    pub source_order: usize,
    pub boundary: SourcePosition,
    pub state: SourceMacroStateId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source::tables) struct SourceMacroStatePositionBoundary {
    pub(in crate::source::tables) source_order: usize,
    pub(in crate::source::tables) boundary: SourcePosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source::tables) struct SourceMacroStateSourceScope {
    pub(in crate::source::tables) end_order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroCall {
    pub id: SourceMacroCallId,
    pub trace_call: Option<MacroCallId>,
    pub trace_expansion: Option<MacroExpansionId>,
    pub parent_trace_expansion: Option<MacroExpansionId>,
    pub reference: SourceMacroReferenceId,
    pub call_range: SourceRange,
    pub callee: SourceMacroResolution,
    pub arguments: Vec<SourceMacroArgument>,
    pub expansion: Option<SourceMacroExpansionId>,
    pub status: SourceMacroCallStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroArgument {
    pub argument_index: usize,
    pub argument_range: Option<SourceRange>,
    pub tokens: Vec<SourceMacroToken>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMacroCallStatus {
    ExpansionAvailable,
    ExpansionUnavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroExpansion {
    pub id: SourceMacroExpansionId,
    pub trace_expansion: Option<MacroExpansionId>,
    pub call: SourceMacroCallId,
    pub definition: SourceMacroExpansionDefinition,
    pub emitted_token_range: SourceEmittedTokenRange,
    pub child_calls: Vec<SourceMacroCallId>,
    pub status: SourceMacroExpansionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMacroExpansionDefinition {
    Source(SourceMacroDefinitionId),
    Builtin { name: SmolStr },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMacroExpansionStatus {
    Complete,
    Unavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMacroExpansionQuery {
    Available(SourceMacroExpansionId),
    Unavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecursiveMacroExpansion {
    pub root_call: SourceMacroCallId,
    pub expansions: Vec<SourceMacroExpansionId>,
    pub unavailable: Vec<SourceMacroExpansionUnavailable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroExpansionUnavailable {
    pub call: SourceMacroCallId,
    pub reason: SourcePreprocUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceEmittedTokenRange {
    pub start: SourceEmittedTokenId,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEmittedToken {
    pub id: SourceEmittedTokenId,
    pub text: SmolStr,
    pub display: SmolStr,
    pub kind: SourceTokenKind,
    pub emitted_range: SourceEmittedTokenRange,
    pub origin: Option<SourceTokenOriginId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTokenOrigin {
    Source {
        token_range: SourceRange,
    },
    MacroBody {
        origin: MacroBodyOrigin,
        definition: SourceMacroDefinitionId,
        body_token_range: SourceRange,
        call: SourceMacroCallId,
    },
    MacroArgument {
        origin: MacroArgumentOrigin,
        call: SourceMacroCallId,
        argument_index: usize,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    },
    TokenPaste {
        origin: MacroOperationOrigin,
        call: SourceMacroCallId,
    },
    Stringification {
        origin: MacroOperationOrigin,
        call: SourceMacroCallId,
    },
    Predefine {
        source: PreprocSourceId,
    },
    Builtin {
        name: SmolStr,
        origin: MacroBuiltinOrigin,
        call: SourceMacroCallId,
    },
}

pub(in crate::source::tables) struct EmittedTokenMacroCall {
    pub(in crate::source::tables) token_id: SourceEmittedTokenId,
    pub(in crate::source::tables) macro_name: SmolStr,
    pub(in crate::source::tables) trace_call: MacroCallId,
    pub(in crate::source::tables) definition: SourceMacroDefinitionId,
    pub(in crate::source::tables) call_range: SourceRange,
    pub(in crate::source::tables) trace_expansion: MacroExpansionId,
    pub(in crate::source::tables) parent_trace_expansion: Option<MacroExpansionId>,
}
