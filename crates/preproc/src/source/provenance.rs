use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::types::*;

macro_rules! source_table_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(usize);

        impl $name {
            pub fn new(raw: usize) -> Self {
                Self(raw)
            }

            pub fn raw(self) -> usize {
                self.0
            }
        }
    };
}

macro_rules! source_table {
    ($table:ident, $field:ident, $id:ident, $item:ty) => {
        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        pub struct $table {
            $field: Vec<$item>,
        }

        impl $table {
            pub fn get(&self, id: $id) -> Option<&$item> {
                self.$field.get(id.raw())
            }

            pub fn iter(&self) -> std::slice::Iter<'_, $item> {
                self.$field.iter()
            }

            pub fn len(&self) -> usize {
                self.$field.len()
            }

            pub fn is_empty(&self) -> bool {
                self.$field.is_empty()
            }

            fn push(&mut self, item: $item) {
                self.$field.push(item);
            }
        }
    };

    ($table:ident, $field:ident, $id:ident, $item:ty,mutable) => {
        source_table!($table, $field, $id, $item);

        impl $table {
            fn get_mut(&mut self, id: $id) -> Option<&mut $item> {
                self.$field.get_mut(id.raw())
            }
        }
    };
}

macro_rules! impl_source_ranges {
    ($ty:ty,directive = $directive:ident) => {
        impl HasDirectiveRange for $ty {
            fn directive_range(&self) -> SourceRange {
                self.$directive
            }
        }
    };

    ($ty:ty,directive = $directive:ident,name = $name:ident) => {
        impl_source_ranges!($ty, directive = $directive);

        impl HasNameRange for $ty {
            fn name_range(&self) -> Option<SourceRange> {
                Some(self.$name)
            }
        }
    };
}

source_table_id!(SourceMacroDefinitionId);
source_table_id!(SourceMacroReferenceId);
source_table_id!(SourceIncludeDirectiveId);
source_table_id!(SourceMacroStateId);
source_table_id!(SourceMacroCallId);
source_table_id!(SourceMacroExpansionId);
source_table_id!(SourceEmittedTokenId);
source_table_id!(SourceTokenProvenanceId);

pub trait HasDirectiveRange {
    fn directive_range(&self) -> SourceRange;
}

pub trait HasNameRange {
    fn name_range(&self) -> Option<SourceRange>;
}

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
    pub identity: Option<SourceMacroDefinitionKey>,
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
    directives: Vec<SourceIncludeDirective>,
    edges: Vec<SourceIncludeEdge>,
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
    states: Vec<SourceMacroState>,
    checkpoints: Vec<SourceMacroStateCheckpoint>,
    source_order_boundaries: BTreeMap<PreprocSourceId, Vec<SourceMacroStatePositionBoundary>>,
    final_source_order: usize,
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
struct SourceMacroStatePositionBoundary {
    source_order: usize,
    boundary: SourcePosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroCall {
    pub id: SourceMacroCallId,
    pub identity: Option<SourceMacroCallKey>,
    pub expansion_identity: Option<SourceMacroExpansionKey>,
    pub parent_expansion_identity: Option<SourceMacroExpansionKey>,
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
    pub identity: Option<SourceMacroExpansionKey>,
    pub call: SourceMacroCallId,
    pub definition: SourceMacroDefinitionId,
    pub emitted_token_range: SourceEmittedTokenRange,
    pub child_calls: Vec<SourceMacroCallId>,
    pub status: SourceMacroExpansionStatus,
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
    pub kind: SourceTokenKind,
    pub emitted_range: SourceEmittedTokenRange,
    pub provenance: SourceTokenProvenanceId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceTokenProvenance {
    Source {
        token_range: SourceRange,
    },
    MacroBody {
        identity: SourceMacroBodyIdentity,
        definition: SourceMacroDefinitionId,
        body_token_range: SourceRange,
        call: SourceMacroCallId,
    },
    MacroArgument {
        identity: SourceMacroArgumentIdentity,
        call: SourceMacroCallId,
        argument_index: usize,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    },
    TokenPaste {
        call: SourceMacroCallId,
        parts: Vec<SourceTokenProvenanceId>,
    },
    Stringification {
        call: SourceMacroCallId,
        argument_index: usize,
    },
    Predefine {
        source: PreprocSourceId,
    },
    Builtin {
        name: SmolStr,
    },
    Unavailable(SourcePreprocUnavailable),
}

struct EmittedTokenMacroCall {
    token_id: SourceEmittedTokenId,
    macro_name: SmolStr,
    call_identity: SourceMacroCallKey,
    definition: SourceMacroDefinitionId,
    call_range: SourceRange,
    expansion_identity: SourceMacroExpansionKey,
    parent_expansion_identity: Option<SourceMacroExpansionKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocTables {
    pub macro_definitions: SourceMacroDefinitionTable,
    pub macro_references: SourceMacroReferenceTable,
    pub macro_calls: SourceMacroCallTable,
    pub macro_expansions: SourceMacroExpansionTable,
    pub emitted_tokens: SourceEmittedTokenTable,
    pub token_provenance: SourceTokenProvenanceTable,
    pub include_graph: SourceIncludeGraph,
    pub inactive_ranges: Vec<SourceRange>,
    pub state_timeline: SourceMacroStateTimeline,
    pub capabilities: SourcePreprocCapabilities,
    pub issues: Vec<SourcePreprocFactIssue>,
}

source_table!(
    SourceMacroDefinitionTable,
    definitions,
    SourceMacroDefinitionId,
    SourceMacroDefinition
);
source_table!(SourceMacroReferenceTable, references, SourceMacroReferenceId, SourceMacroReference);
source_table!(SourceMacroCallTable, calls, SourceMacroCallId, SourceMacroCall, mutable);
source_table!(SourceMacroExpansionTable, expansions, SourceMacroExpansionId, SourceMacroExpansion);
source_table!(SourceEmittedTokenTable, tokens, SourceEmittedTokenId, SourceEmittedToken);
source_table!(
    SourceTokenProvenanceTable,
    provenance,
    SourceTokenProvenanceId,
    SourceTokenProvenance
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocCapabilities {
    pub source_events: CapabilityStatus,
    pub definition_name_ranges: CapabilityStatus,
    pub include_edges: CapabilityStatus,
    pub inactive_ranges: CapabilityStatus,
    pub macro_reference_resolution: CapabilityStatus,
    pub macro_calls: CapabilityStatus,
    pub macro_expansions: CapabilityStatus,
    pub emitted_tokens: CapabilityStatus,
    pub emitted_token_provenance: CapabilityStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    Complete,
    Partial,
    Unavailable(SourcePreprocUnavailable),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocUnavailable {
    MissingDefinitionName { event_id: SourcePreprocEventId },
    MissingDefinitionNameRange { event_id: SourcePreprocEventId },
    MissingReferenceName { event_id: SourcePreprocEventId },
    MissingReferenceNameRange { event_id: SourcePreprocEventId },
    DetachedSource { source: PreprocSourceId },
    MissingPredefineSourceText { source: PreprocSourceId },
    UnverifiedPredefineSource { source: PreprocSourceId },
    MacroCallAuthorityUnavailable,
    EmittedTokenAuthorityUnavailable,
    TokenProvenanceAuthorityUnavailable,
    ExpansionAuthorityUnavailable,
    MissingMacroCall { call: SourceMacroCallId },
    MissingMacroExpansion { call: SourceMacroCallId },
    MissingEmittedTokenMacroCall { source: PreprocSourceId },
    UnknownMacroUsageDefinitionIdentity { identity: SourceMacroDefinitionKey },
    MissingEmittedTokenMacroCallIdentity,
    UnknownEmittedTokenMacroCallIdentity { identity: SourceMacroCallKey },
    MissingEmittedTokenMacroDefinitionIdentity,
    UnknownEmittedTokenMacroDefinitionIdentity { identity: SourceMacroDefinitionKey },
    MissingEmittedTokenMacroExpansionIdentity { call: SourceMacroCallId },
    UnmappedParentMacroExpansionIdentity { identity: SourceMacroExpansionKey },
    MissingEmittedTokenMacroDefinition { call: SourceMacroCallId },
    MissingEmittedTokenMacroBody { call: SourceMacroCallId },
    MissingEmittedTokenMacroArgument { call: SourceMacroCallId },
    NonContiguousEmittedTokenRange { call: SourceMacroCallId },
    UnsupportedEmittedTokenProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocFactIssue {
    MissingDefinitionName { event_id: SourcePreprocEventId },
    MissingDefinitionNameRange { event_id: SourcePreprocEventId },
    MissingReferenceName { event_id: SourcePreprocEventId },
    MissingReferenceNameRange { event_id: SourcePreprocEventId },
    DetachedSource { source: PreprocSourceId },
}

impl SourcePreprocTables {
    pub fn from_index(index: &SourcePreprocIndex) -> Self {
        SourcePreprocModelBuilder::new(index).build()
    }

    pub fn capabilities(&self) -> &SourcePreprocCapabilities {
        &self.capabilities
    }
}

impl Default for SourcePreprocTables {
    fn default() -> Self {
        Self {
            macro_definitions: SourceMacroDefinitionTable::default(),
            macro_references: SourceMacroReferenceTable::default(),
            macro_calls: SourceMacroCallTable::default(),
            macro_expansions: SourceMacroExpansionTable::default(),
            emitted_tokens: SourceEmittedTokenTable::default(),
            token_provenance: SourceTokenProvenanceTable::default(),
            include_graph: SourceIncludeGraph::default(),
            inactive_ranges: Vec::new(),
            state_timeline: SourceMacroStateTimeline::default(),
            capabilities: SourcePreprocCapabilities::unavailable(),
            issues: Vec::new(),
        }
    }
}

impl SourceIncludeGraph {
    pub fn directives(&self) -> &[SourceIncludeDirective] {
        &self.directives
    }

    pub fn edges(&self) -> &[SourceIncludeEdge] {
        &self.edges
    }
}

impl SourceMacroStateTimeline {
    pub fn states(&self) -> &[SourceMacroState] {
        &self.states
    }

    pub fn checkpoints(&self) -> &[SourceMacroStateCheckpoint] {
        &self.checkpoints
    }
}

impl SourceMacroStateTimeline {
    pub fn state_at_position(&self, position: SourcePosition) -> Option<&SourceMacroState> {
        let source_order = self.source_order_at_position(position);
        self.state_at_source_order(source_order)
    }

    fn source_order_at_position(&self, position: SourcePosition) -> usize {
        let Some(boundaries) = self.source_order_boundaries.get(&position.source) else {
            return self.final_source_order;
        };
        let index =
            boundaries.partition_point(|boundary| boundary.boundary.offset <= position.offset);
        boundaries
            .get(index)
            .map(|boundary| boundary.source_order)
            .unwrap_or(self.final_source_order)
    }

    fn state_at_source_order(&self, source_order: usize) -> Option<&SourceMacroState> {
        let index =
            self.checkpoints.partition_point(|checkpoint| checkpoint.source_order <= source_order);
        if index == 0 {
            return None;
        }
        let checkpoint = &self.checkpoints[index - 1];
        self.states.get(checkpoint.state.raw())
    }
}

impl SourcePreprocCapabilities {
    pub fn unavailable() -> Self {
        Self {
            source_events: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
            definition_name_ranges: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
            include_edges: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
            inactive_ranges: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
            macro_reference_resolution: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
            macro_calls: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::MacroCallAuthorityUnavailable,
            ),
            macro_expansions: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
            emitted_tokens: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable,
            ),
            emitted_token_provenance: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable,
            ),
        }
    }
}

impl_source_ranges!(SourceMacroDefinition, directive = directive_range, name = name_range);
impl_source_ranges!(SourceMacroReference, directive = directive_range, name = name_range);
impl_source_ranges!(SourceIncludeDirective, directive = directive_range);

mod builder;
pub use builder::SourcePreprocModelBuilder;
