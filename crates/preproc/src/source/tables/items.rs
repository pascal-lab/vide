use super::*;

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
    pub name: SmolStr,
    pub name_range: SourceRange,
    pub directive_range: SourceRange,
    pub resolution: SourceMacroResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceMacroResolution {
    Resolved { definition: SourceMacroDefinitionId },
    Undefined,
    Unavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceIncludeGraph {
    pub(in crate::source::tables) directives: Vec<SourceIncludeDirective>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceMacroStateCheckpoint {
    pub source_order: usize,
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
    pub reference: SourceMacroReferenceId,
    pub call_range: SourceRange,
    pub arguments: Vec<SourceMacroArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroArgument {
    pub argument_index: usize,
    pub argument_range: Option<SourceRange>,
    pub tokens: Vec<SourceMacroToken>,
}
