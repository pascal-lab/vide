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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroDefinitionTable {
    definitions: Vec<SourceMacroDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroReferenceTable {
    references: Vec<SourceMacroReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroCallTable {
    calls: Vec<SourceMacroCall>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceMacroExpansionTable {
    expansions: Vec<SourceMacroExpansion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceEmittedTokenTable {
    tokens: Vec<SourceEmittedToken>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceTokenProvenanceTable {
    provenance: Vec<SourceTokenProvenance>,
}

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
    MissingIncludedSource { include_event_id: SourcePreprocEventId, source: PreprocSourceId },
    MissingIncludeEvent { include_event_id: SourcePreprocEventId },
    IncludeEdgeNotInclude { include_event_id: SourcePreprocEventId },
    IncludeChainUnavailable { source: PreprocSourceId },
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
    MissingIncludedSource { include_event_id: SourcePreprocEventId, source: PreprocSourceId },
    MissingIncludeEvent { include_event_id: SourcePreprocEventId },
    IncludeEdgeNotInclude { include_event_id: SourcePreprocEventId },
    IncludeChainUnavailable { source: PreprocSourceId },
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

impl SourceMacroDefinitionTable {
    pub fn get(&self, id: SourceMacroDefinitionId) -> Option<&SourceMacroDefinition> {
        self.definitions.get(id.raw())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceMacroDefinition> {
        self.definitions.iter()
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    fn push(&mut self, definition: SourceMacroDefinition) {
        self.definitions.push(definition);
    }
}

impl SourceMacroReferenceTable {
    pub fn get(&self, id: SourceMacroReferenceId) -> Option<&SourceMacroReference> {
        self.references.get(id.raw())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceMacroReference> {
        self.references.iter()
    }

    pub fn len(&self) -> usize {
        self.references.len()
    }

    pub fn is_empty(&self) -> bool {
        self.references.is_empty()
    }

    fn push(&mut self, reference: SourceMacroReference) {
        self.references.push(reference);
    }
}

impl SourceMacroCallTable {
    pub fn get(&self, id: SourceMacroCallId) -> Option<&SourceMacroCall> {
        self.calls.get(id.raw())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceMacroCall> {
        self.calls.iter()
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }

    fn push(&mut self, call: SourceMacroCall) {
        self.calls.push(call);
    }

    fn get_mut(&mut self, id: SourceMacroCallId) -> Option<&mut SourceMacroCall> {
        self.calls.get_mut(id.raw())
    }
}

impl SourceMacroExpansionTable {
    pub fn get(&self, id: SourceMacroExpansionId) -> Option<&SourceMacroExpansion> {
        self.expansions.get(id.raw())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceMacroExpansion> {
        self.expansions.iter()
    }

    pub fn len(&self) -> usize {
        self.expansions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.expansions.is_empty()
    }

    fn push(&mut self, expansion: SourceMacroExpansion) {
        self.expansions.push(expansion);
    }
}

impl SourceEmittedTokenTable {
    pub fn get(&self, id: SourceEmittedTokenId) -> Option<&SourceEmittedToken> {
        self.tokens.get(id.raw())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceEmittedToken> {
        self.tokens.iter()
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    fn push(&mut self, token: SourceEmittedToken) {
        self.tokens.push(token);
    }
}

impl SourceTokenProvenanceTable {
    pub fn get(&self, id: SourceTokenProvenanceId) -> Option<&SourceTokenProvenance> {
        self.provenance.get(id.raw())
    }

    pub fn iter(&self) -> impl Iterator<Item = &SourceTokenProvenance> {
        self.provenance.iter()
    }

    pub fn len(&self) -> usize {
        self.provenance.len()
    }

    pub fn is_empty(&self) -> bool {
        self.provenance.is_empty()
    }

    fn push(&mut self, provenance: SourceTokenProvenance) {
        self.provenance.push(provenance);
    }
}

impl HasDirectiveRange for SourceMacroDefinition {
    fn directive_range(&self) -> SourceRange {
        self.directive_range
    }
}

impl HasNameRange for SourceMacroDefinition {
    fn name_range(&self) -> Option<SourceRange> {
        Some(self.name_range)
    }
}

impl HasDirectiveRange for SourceMacroReference {
    fn directive_range(&self) -> SourceRange {
        self.directive_range
    }
}

impl HasNameRange for SourceMacroReference {
    fn name_range(&self) -> Option<SourceRange> {
        Some(self.name_range)
    }
}

impl HasDirectiveRange for SourceIncludeDirective {
    fn directive_range(&self) -> SourceRange {
        self.directive_range
    }
}

pub struct SourcePreprocModelBuilder<'a> {
    index: &'a SourcePreprocIndex,
    tables: SourcePreprocTables,
    definition_ids_by_define_index: BTreeMap<usize, SourceMacroDefinitionId>,
    definition_ids_by_identity: BTreeMap<SourceMacroDefinitionKey, SourceMacroDefinitionId>,
    call_ids_by_identity: BTreeMap<SourceMacroCallKey, SourceMacroCallId>,
    call_ids_by_expansion_identity: BTreeMap<SourceMacroExpansionKey, SourceMacroCallId>,
    current_state: BTreeMap<SmolStr, SourceMacroDefinitionId>,
    definition_ranges_partial: bool,
    include_edges_partial: bool,
    references_partial: bool,
    macro_calls_partial: bool,
    token_provenance_partial: bool,
    expansions_partial: bool,
}

impl<'a> SourcePreprocModelBuilder<'a> {
    pub fn new(index: &'a SourcePreprocIndex) -> Self {
        Self {
            index,
            tables: SourcePreprocTables::default(),
            definition_ids_by_define_index: BTreeMap::new(),
            definition_ids_by_identity: BTreeMap::new(),
            call_ids_by_identity: BTreeMap::new(),
            call_ids_by_expansion_identity: BTreeMap::new(),
            current_state: BTreeMap::new(),
            definition_ranges_partial: false,
            include_edges_partial: false,
            references_partial: false,
            macro_calls_partial: false,
            token_provenance_partial: false,
            expansions_partial: false,
        }
    }

    pub fn build(mut self) -> SourcePreprocTables {
        self.build_tables();
        self.tables
    }

    fn build_tables(&mut self) {
        self.build_definition_table();
        self.build_include_graph();
        self.record_position_boundaries();
        self.record_state_checkpoint(0, SourcePosition::from_first_event(self.index));
        self.scan_references_and_state();
        self.build_emitted_token_tables();
        self.build_macro_expansion_graph();
        self.record_macro_body_references_for_calls();
        let macro_expansions = if self.tables.macro_calls.is_empty() {
            CapabilityStatus::Complete
        } else if self.index.emitted_tokens.is_empty() {
            CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable,
            )
        } else {
            partial_status(self.expansions_partial)
        };
        self.tables.capabilities = SourcePreprocCapabilities {
            source_events: CapabilityStatus::Complete,
            definition_name_ranges: partial_status(self.definition_ranges_partial),
            include_edges: partial_status(self.include_edges_partial),
            inactive_ranges: CapabilityStatus::Complete,
            macro_reference_resolution: partial_status(self.references_partial),
            macro_calls: partial_status(self.references_partial || self.macro_calls_partial),
            macro_expansions,
            emitted_tokens: CapabilityStatus::Complete,
            emitted_token_provenance: partial_status(self.token_provenance_partial),
        };
    }

    fn build_definition_table(&mut self) {
        for (define_index, define) in self.index.defines.iter().enumerate() {
            let Some(name) = define.name.clone() else {
                self.definition_ranges_partial = true;
                self.tables.issues.push(SourcePreprocFactIssue::MissingDefinitionName {
                    event_id: define.event_id,
                });
                continue;
            };
            let Some(name_range) = define.name_range else {
                self.definition_ranges_partial = true;
                self.tables.issues.push(SourcePreprocFactIssue::MissingDefinitionNameRange {
                    event_id: define.event_id,
                });
                continue;
            };

            let id = SourceMacroDefinitionId::new(self.tables.macro_definitions.len());
            self.tables.macro_definitions.push(SourceMacroDefinition {
                id,
                event_id: define.event_id,
                identity: define.identity,
                name,
                name_range,
                directive_range: define.range,
                params: define.params.clone(),
                body_tokens: define.body.clone(),
            });
            self.definition_ids_by_define_index.insert(define_index, id);
            if let Some(identity) = define.identity {
                self.definition_ids_by_identity.insert(identity, id);
            }
        }
    }

    fn record_position_boundaries(&mut self) {
        self.tables.state_timeline.final_source_order = self.index.event_records.len();
        for (source_order, directive) in self.index.event_records.iter().enumerate() {
            self.tables
                .state_timeline
                .source_order_boundaries
                .entry(directive.range.source)
                .or_default()
                .push(SourceMacroStatePositionBoundary {
                    source_order,
                    boundary: boundary_after(directive.range),
                });
        }

        for boundaries in self.tables.state_timeline.source_order_boundaries.values_mut() {
            boundaries.sort_by_key(|boundary| (boundary.boundary.offset, boundary.source_order));
        }
    }

    fn build_include_graph(&mut self) {
        self.tables.inactive_ranges = self.index.inactive_ranges.clone();
        let mut resolved_sources_by_event = BTreeMap::new();
        let mut unavailable_by_event = BTreeMap::new();
        let mut valid_edges = Vec::new();

        for edge in &self.index.include_edges {
            if let Some(unavailable) = self.validate_include_edge(edge) {
                unavailable_by_event.insert(edge.include_event_id, unavailable);
                continue;
            }

            valid_edges.push(*edge);
            resolved_sources_by_event.insert(edge.include_event_id, edge.included_source);
        }

        for source in &self.index.sources {
            if source.origin == PreprocSourceOrigin::Detached {
                self.include_edges_partial = true;
                self.tables
                    .issues
                    .push(SourcePreprocFactIssue::DetachedSource { source: source.id });
            }
        }

        self.tables.include_graph.edges = valid_edges;
        for include in &self.index.includes {
            let id = SourceIncludeDirectiveId::new(self.tables.include_graph.directives.len());
            let resolved_source = resolved_sources_by_event.get(&include.event_id).copied();
            let status = match resolved_source {
                Some(source) => SourceIncludeStatus::Resolved { source },
                None => unavailable_by_event
                    .remove(&include.event_id)
                    .map(SourceIncludeStatus::Unavailable)
                    .unwrap_or(SourceIncludeStatus::Unresolved),
            };
            self.tables.include_graph.directives.push(SourceIncludeDirective {
                id,
                event_id: include.event_id,
                directive_range: include.range,
                target: include.target.clone(),
                target_range: include.target_range,
                resolved_source,
                status,
            });
        }
    }

    fn validate_include_edge(
        &mut self,
        edge: &SourceIncludeEdge,
    ) -> Option<SourcePreprocUnavailable> {
        if !self.index.sources.iter().any(|source| source.id == edge.included_source) {
            self.include_edges_partial = true;
            self.tables.issues.push(SourcePreprocFactIssue::MissingIncludedSource {
                include_event_id: edge.include_event_id,
                source: edge.included_source,
            });
            return Some(SourcePreprocUnavailable::MissingIncludedSource {
                include_event_id: edge.include_event_id,
                source: edge.included_source,
            });
        }

        let Some(directive) = self
            .index
            .event_records
            .iter()
            .find(|directive| directive.event_id == edge.include_event_id)
        else {
            self.include_edges_partial = true;
            self.tables.issues.push(SourcePreprocFactIssue::MissingIncludeEvent {
                include_event_id: edge.include_event_id,
            });
            return Some(SourcePreprocUnavailable::MissingIncludeEvent {
                include_event_id: edge.include_event_id,
            });
        };

        if directive.kind != MacroEventKind::Include {
            self.include_edges_partial = true;
            self.tables.issues.push(SourcePreprocFactIssue::IncludeEdgeNotInclude {
                include_event_id: edge.include_event_id,
            });
            return Some(SourcePreprocUnavailable::IncludeEdgeNotInclude {
                include_event_id: edge.include_event_id,
            });
        }

        None
    }

    fn scan_references_and_state(&mut self) {
        for (source_order, directive) in self.index.event_records.iter().enumerate() {
            match directive.kind {
                MacroEventKind::Define => self.apply_define(source_order, directive),
                MacroEventKind::Undef => self.apply_undef(source_order, directive),
                MacroEventKind::Conditional => self.record_conditional_references(directive),
                MacroEventKind::Usage => self.record_usage_reference(directive),
                MacroEventKind::Include | MacroEventKind::Branch => {}
            }
        }
    }

    fn apply_define(&mut self, source_order: usize, directive: &SourcePreprocEventRecord) {
        if let Some(definition_id) = self.definition_ids_by_define_index.get(&directive.index) {
            let definition = self
                .tables
                .macro_definitions
                .get(*definition_id)
                .expect("definition id should point at inserted definition");
            self.current_state.insert(definition.name.clone(), *definition_id);
            self.record_state_checkpoint(source_order + 1, boundary_after(directive.range));
        }
    }

    fn apply_undef(&mut self, source_order: usize, directive: &SourcePreprocEventRecord) {
        let Some(undef) = self.index.undefs.get(directive.index) else {
            return;
        };
        if let Some(name) = undef.name.as_ref() {
            self.current_state.remove(name.as_str());
            self.record_state_checkpoint(source_order + 1, boundary_after(directive.range));
        }
    }

    fn record_usage_reference(&mut self, directive: &SourcePreprocEventRecord) {
        let Some(usage) = self.index.usages.get(directive.index) else {
            return;
        };
        let Some(name) = usage.name.clone() else {
            self.record_missing_reference_name(usage.event_id);
            return;
        };
        let Some(name_range) = usage.name_range else {
            self.record_missing_reference_name_range(usage.event_id);
            return;
        };
        let event_id = usage.event_id;
        let directive_range = usage.range;
        let resolution = self.resolve_visible_reference(name.as_str());
        let reference = self.push_reference(
            event_id,
            SourceMacroReferenceSite::Usage { usage_index: directive.index },
            name.clone(),
            name_range,
            directive_range,
            resolution.clone(),
        );
        self.push_call(reference, directive_range, resolution, usage.identity, None, None);
    }

    fn record_conditional_references(&mut self, directive: &SourcePreprocEventRecord) {
        let Some(conditional) = self.index.conditionals.get(directive.index) else {
            return;
        };
        let event_id = conditional.event_id;
        let directive_range = conditional.range;
        for (token_index, token) in conditional.expr.iter().enumerate() {
            let name = token.value.clone();
            let Some(name_range) = token.range else {
                self.record_missing_reference_name_range(event_id);
                continue;
            };
            let (site, resolution) =
                if let Some(definition) = self.current_state.get(name.as_str()).copied() {
                    (
                        SourceMacroReferenceSite::ConditionalToken {
                            conditional_index: directive.index,
                            token_index,
                        },
                        self.resolve_definition(
                            definition,
                            SourceMacroResolutionReason::VisibleDefinition,
                        ),
                    )
                } else if let Some(definition) =
                    self.include_guard_definition_after_ifndef(directive.index, name.as_str())
                {
                    (
                        SourceMacroReferenceSite::IncludeGuardIfNDef {
                            conditional_index: directive.index,
                            token_index,
                        },
                        self.resolve_definition(
                            definition,
                            SourceMacroResolutionReason::IncludeGuardIfNDef,
                        ),
                    )
                } else {
                    (
                        SourceMacroReferenceSite::ConditionalToken {
                            conditional_index: directive.index,
                            token_index,
                        },
                        SourceMacroResolution::Undefined,
                    )
                };
            self.push_reference(event_id, site, name, name_range, directive_range, resolution);
        }
    }

    fn push_reference(
        &mut self,
        event_id: SourcePreprocEventId,
        site: SourceMacroReferenceSite,
        name: SmolStr,
        name_range: SourceRange,
        directive_range: SourceRange,
        resolution: SourceMacroResolution,
    ) -> SourceMacroReferenceId {
        let id = SourceMacroReferenceId::new(self.tables.macro_references.len());
        self.tables.macro_references.push(SourceMacroReference {
            id,
            event_id,
            site,
            name,
            name_range,
            directive_range,
            resolution,
        });
        id
    }

    fn push_call(
        &mut self,
        reference: SourceMacroReferenceId,
        call_range: SourceRange,
        callee: SourceMacroResolution,
        identity: Option<SourceMacroCallKey>,
        expansion_identity: Option<SourceMacroExpansionKey>,
        parent_expansion_identity: Option<SourceMacroExpansionKey>,
    ) -> SourceMacroCallId {
        let id = SourceMacroCallId::new(self.tables.macro_calls.len());
        self.tables.macro_calls.push(SourceMacroCall {
            id,
            identity,
            expansion_identity,
            parent_expansion_identity,
            reference,
            call_range,
            callee,
            arguments: Vec::new(),
            expansion: None,
            status: SourceMacroCallStatus::ExpansionUnavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
        });
        if let Some(identity) = identity {
            self.call_ids_by_identity.insert(identity, id);
        } else {
            self.macro_calls_partial = true;
        }
        if let Some(expansion_identity) = expansion_identity {
            self.call_ids_by_expansion_identity.insert(expansion_identity, id);
        }
        id
    }

    fn build_emitted_token_tables(&mut self) {
        for index in 0..self.index.emitted_tokens.len() {
            let token = self.index.emitted_tokens[index].clone();
            let token_id = SourceEmittedTokenId::new(self.tables.emitted_tokens.len());
            let provenance = self.resolve_emitted_token_provenance(token_id, &token);
            let provenance_id = SourceTokenProvenanceId::new(self.tables.token_provenance.len());
            self.tables.token_provenance.push(provenance);

            self.tables.emitted_tokens.push(SourceEmittedToken {
                id: token_id,
                text: token.raw,
                kind: token.kind,
                emitted_range: SourceEmittedTokenRange { start: token_id, len: 1 },
                provenance: provenance_id,
            });
        }
    }

    fn resolve_emitted_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        token: &SourceEmittedTokenFact,
    ) -> SourceTokenProvenance {
        match &token.provenance {
            SourceTokenProvenanceFact::Source { token_range } => {
                SourceTokenProvenance::Source { token_range: *token_range }
            }
            SourceTokenProvenanceFact::MacroBody {
                macro_name,
                identity,
                call_range,
                body_token_range,
            } => self.resolve_macro_body_token_provenance(
                token_id,
                macro_name.clone(),
                *identity,
                *call_range,
                *body_token_range,
            ),
            SourceTokenProvenanceFact::MacroArgument {
                macro_name,
                identity,
                call_range,
                body_token_range,
                argument_token_range,
            } => self.resolve_macro_argument_token_provenance(
                token_id,
                macro_name.clone(),
                *identity,
                *call_range,
                *body_token_range,
                *argument_token_range,
            ),
            SourceTokenProvenanceFact::Builtin { name } if !name.is_empty() => {
                SourceTokenProvenance::Builtin { name: name.clone() }
            }
            SourceTokenProvenanceFact::Builtin { .. } | SourceTokenProvenanceFact::Unavailable => {
                self.unavailable_token_provenance(
                    SourcePreprocUnavailable::UnsupportedEmittedTokenProvenance,
                )
            }
        }
    }

    fn resolve_macro_body_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        macro_name: SmolStr,
        identity: Option<SourceMacroBodyIdentity>,
        call_range: SourceRange,
        body_token_range: SourceRange,
    ) -> SourceTokenProvenance {
        if self.source_is_predefine(body_token_range.source) {
            return SourceTokenProvenance::Predefine { source: body_token_range.source };
        }

        let Some(identity) = identity else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroCallIdentity,
            );
        };
        let Ok(definition) = self.definition_for_identity(identity.definition) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::UnknownEmittedTokenMacroDefinitionIdentity {
                    identity: identity.definition,
                },
            );
        };
        let Ok(call) = self.call_for_emitted_token(
            token_id,
            macro_name,
            identity.call,
            definition,
            call_range,
            identity.expansion,
            identity.parent_expansion,
        ) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::UnknownEmittedTokenMacroCallIdentity {
                    identity: identity.call,
                },
            );
        };

        if !self.definition_body_token_exists(definition, identity.body_token_index) {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroBody { call },
            );
        }

        SourceTokenProvenance::MacroBody { identity, definition, body_token_range, call }
    }

    fn resolve_macro_argument_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        macro_name: SmolStr,
        identity: Option<SourceMacroArgumentIdentity>,
        call_range: SourceRange,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    ) -> SourceTokenProvenance {
        let Some(identity) = identity else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroCallIdentity,
            );
        };
        let Ok(definition) = self.definition_for_identity(identity.definition) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::UnknownEmittedTokenMacroDefinitionIdentity {
                    identity: identity.definition,
                },
            );
        };
        let call_expansion_identity = identity.parent_expansion.unwrap_or(identity.expansion);
        let Ok(call) = self.call_for_emitted_token(
            token_id,
            macro_name,
            identity.call,
            definition,
            call_range,
            call_expansion_identity,
            None,
        ) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::UnknownEmittedTokenMacroCallIdentity {
                    identity: identity.call,
                },
            );
        };
        if !self.definition_body_token_exists(definition, identity.body_token_index) {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroBody { call },
            );
        }
        if !self.definition_parameter_exists(definition, identity.argument_index) {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroArgument { call },
            );
        };
        self.record_macro_argument(call, identity.argument_index, argument_token_range);

        SourceTokenProvenance::MacroArgument {
            identity,
            call,
            argument_index: identity.argument_index,
            body_token_range,
            argument_token_range,
        }
    }

    fn call_for_emitted_token(
        &mut self,
        token_id: SourceEmittedTokenId,
        macro_name: SmolStr,
        call_identity: SourceMacroCallKey,
        definition: SourceMacroDefinitionId,
        call_range: SourceRange,
        expansion_identity: SourceMacroExpansionKey,
        parent_expansion_identity: Option<SourceMacroExpansionKey>,
    ) -> Result<SourceMacroCallId, SourcePreprocUnavailable> {
        if let Some(call) = self.call_ids_by_identity.get(&call_identity).copied() {
            self.record_call_expansion_identity(
                call,
                expansion_identity,
                parent_expansion_identity,
            )?;
            return Ok(call);
        }

        let event_id = self
            .tables
            .macro_definitions
            .get(definition)
            .expect("definition id should point at inserted definition")
            .event_id;
        let resolution =
            self.resolve_definition(definition, SourceMacroResolutionReason::VisibleDefinition);
        let reference = self.push_reference(
            event_id,
            SourceMacroReferenceSite::ExpansionToken { emitted_token: token_id },
            macro_name.clone(),
            call_range,
            call_range,
            resolution.clone(),
        );
        Ok(self.push_call(
            reference,
            call_range,
            resolution,
            Some(call_identity),
            Some(expansion_identity),
            parent_expansion_identity,
        ))
    }

    fn definition_for_call(&self, call: SourceMacroCallId) -> Result<SourceMacroDefinitionId, ()> {
        let Some(call) = self.tables.macro_calls.get(call) else {
            return Err(());
        };
        match &call.callee {
            SourceMacroResolution::Resolved { definition, .. } => Ok(*definition),
            SourceMacroResolution::Undefined | SourceMacroResolution::Unavailable(_) => Err(()),
        }
    }

    fn definition_for_identity(
        &self,
        identity: SourceMacroDefinitionKey,
    ) -> Result<SourceMacroDefinitionId, ()> {
        self.definition_ids_by_identity.get(&identity).copied().ok_or(())
    }

    fn definition_body_token_exists(
        &self,
        definition: SourceMacroDefinitionId,
        body_token_index: usize,
    ) -> bool {
        let Some(definition) = self.tables.macro_definitions.get(definition) else {
            return false;
        };
        definition.body_tokens.get(body_token_index).is_some()
    }

    fn definition_parameter_exists(
        &self,
        definition: SourceMacroDefinitionId,
        argument_index: usize,
    ) -> bool {
        let Some(definition) = self.tables.macro_definitions.get(definition) else {
            return false;
        };
        definition.params.as_ref().is_some_and(|params| params.get(argument_index).is_some())
    }

    fn record_call_expansion_identity(
        &mut self,
        call: SourceMacroCallId,
        expansion_identity: SourceMacroExpansionKey,
        parent_expansion_identity: Option<SourceMacroExpansionKey>,
    ) -> Result<(), SourcePreprocUnavailable> {
        let Some(call_fact) = self.tables.macro_calls.get_mut(call) else {
            return Err(SourcePreprocUnavailable::MissingMacroCall { call });
        };
        if let Some(existing) = call_fact.expansion_identity {
            if existing != expansion_identity {
                self.expansions_partial = true;
                return Err(SourcePreprocUnavailable::MissingEmittedTokenMacroExpansionIdentity {
                    call,
                });
            }
        } else {
            call_fact.expansion_identity = Some(expansion_identity);
            self.call_ids_by_expansion_identity.insert(expansion_identity, call);
        }
        if let Some(parent_expansion_identity) = parent_expansion_identity {
            match call_fact.parent_expansion_identity {
                Some(existing) if existing != parent_expansion_identity => {
                    self.expansions_partial = true;
                    return Err(SourcePreprocUnavailable::UnmappedParentMacroExpansionIdentity {
                        identity: parent_expansion_identity,
                    });
                }
                Some(_) => {}
                None => call_fact.parent_expansion_identity = Some(parent_expansion_identity),
            }
        }
        Ok(())
    }

    fn record_macro_argument(
        &mut self,
        call: SourceMacroCallId,
        argument_index: usize,
        argument_token_range: SourceRange,
    ) {
        let Some(call) = self.tables.macro_calls.get_mut(call) else {
            return;
        };
        if let Some(argument) =
            call.arguments.iter_mut().find(|argument| argument.argument_index == argument_index)
        {
            argument.argument_range =
                merge_source_ranges(argument.argument_range, argument_token_range);
            return;
        }
        call.arguments.push(SourceMacroArgument {
            argument_index,
            argument_range: Some(argument_token_range),
            tokens: Vec::new(),
        });
        call.arguments.sort_by_key(|argument| argument.argument_index);
    }

    fn build_macro_expansion_graph(&mut self) {
        if self.tables.macro_calls.is_empty() {
            return;
        }

        if self.index.emitted_tokens.is_empty() {
            self.mark_all_calls_unavailable(
                SourcePreprocUnavailable::EmittedTokenAuthorityUnavailable,
            );
            return;
        }

        let direct_tokens_by_call = self.direct_emitted_tokens_by_call();
        let child_calls_by_parent = self.child_calls_by_parent();
        let call_ids = self.tables.macro_calls.iter().map(|call| call.id).collect::<Vec<_>>();
        let mut expansion_tokens_by_call = BTreeMap::new();
        for call in &call_ids {
            let mut visiting = Vec::new();
            let tokens = self.recursive_emitted_tokens_for_call(
                *call,
                &direct_tokens_by_call,
                &child_calls_by_parent,
                &mut visiting,
            );
            expansion_tokens_by_call.insert(*call, tokens);
        }

        for call in call_ids {
            let tokens = expansion_tokens_by_call.remove(&call).unwrap_or_default();
            let Some(expansion_identity) =
                self.tables.macro_calls.get(call).and_then(|call| call.expansion_identity)
            else {
                self.mark_call_unavailable(
                    call,
                    SourcePreprocUnavailable::MissingEmittedTokenMacroExpansionIdentity { call },
                );
                continue;
            };
            let Some(emitted_token_range) = emitted_token_range_from_ids(&tokens) else {
                self.mark_call_unavailable(
                    call,
                    if tokens.is_empty() {
                        SourcePreprocUnavailable::ExpansionAuthorityUnavailable
                    } else {
                        SourcePreprocUnavailable::NonContiguousEmittedTokenRange { call }
                    },
                );
                continue;
            };
            let Ok(definition) = self.definition_for_call(call) else {
                self.mark_call_unavailable(
                    call,
                    SourcePreprocUnavailable::MissingEmittedTokenMacroDefinition { call },
                );
                continue;
            };

            let expansion = SourceMacroExpansionId::new(self.tables.macro_expansions.len());
            self.tables.macro_expansions.push(SourceMacroExpansion {
                id: expansion,
                identity: Some(expansion_identity),
                call,
                definition,
                emitted_token_range,
                child_calls: child_calls_by_parent.get(&call).cloned().unwrap_or_default(),
                status: SourceMacroExpansionStatus::Complete,
            });
            if let Some(call) = self.tables.macro_calls.get_mut(call) {
                call.expansion = Some(expansion);
                call.status = SourceMacroCallStatus::ExpansionAvailable;
            }
        }
    }

    fn record_macro_body_references_for_calls(&mut self) {
        let calls = self.tables.macro_calls.iter().cloned().collect::<Vec<_>>();
        for call in calls {
            let SourceMacroResolution::Resolved { definition, .. } = call.callee else {
                continue;
            };
            let Some(definition) = self.tables.macro_definitions.get(definition).cloned() else {
                continue;
            };
            let call_position = SourcePosition {
                source: call.call_range.source,
                offset: call.call_range.range.start(),
            };
            for (token_index, token) in definition.body_tokens.iter().enumerate() {
                let Some(name) = macro_reference_name_from_body_token(token) else {
                    continue;
                };
                let Some(name_range) = token.range else {
                    self.record_missing_reference_name_range(definition.event_id);
                    continue;
                };
                let resolution =
                    self.resolve_visible_reference_at_position(name.as_str(), call_position);
                if self.macro_reference_exists(name.as_str(), name_range, &resolution) {
                    continue;
                }
                self.push_reference(
                    definition.event_id,
                    SourceMacroReferenceSite::MacroBodyToken { call: call.id, token_index },
                    name,
                    name_range,
                    definition.directive_range,
                    resolution,
                );
            }
        }
    }

    fn macro_reference_exists(
        &self,
        name: &str,
        name_range: SourceRange,
        resolution: &SourceMacroResolution,
    ) -> bool {
        self.tables.macro_references.iter().any(|reference| {
            reference.name.as_str() == name
                && reference.name_range == name_range
                && &reference.resolution == resolution
        })
    }

    fn direct_emitted_tokens_by_call(
        &self,
    ) -> BTreeMap<SourceMacroCallId, Vec<SourceEmittedTokenId>> {
        let mut tokens_by_call = BTreeMap::<SourceMacroCallId, Vec<SourceEmittedTokenId>>::new();
        for token in self.tables.emitted_tokens.iter() {
            let Some(provenance) = self.tables.token_provenance.get(token.provenance) else {
                continue;
            };
            let call = match provenance {
                SourceTokenProvenance::MacroBody { call, .. }
                | SourceTokenProvenance::MacroArgument { call, .. }
                | SourceTokenProvenance::TokenPaste { call, .. }
                | SourceTokenProvenance::Stringification { call, .. } => *call,
                SourceTokenProvenance::Source { .. }
                | SourceTokenProvenance::Predefine { .. }
                | SourceTokenProvenance::Builtin { .. }
                | SourceTokenProvenance::Unavailable(_) => continue,
            };
            tokens_by_call.entry(call).or_default().push(token.id);
        }
        tokens_by_call
    }

    fn child_calls_by_parent(&mut self) -> BTreeMap<SourceMacroCallId, Vec<SourceMacroCallId>> {
        let call_ids = self.tables.macro_calls.iter().map(|call| call.id).collect::<Vec<_>>();
        let mut children = BTreeMap::<SourceMacroCallId, Vec<SourceMacroCallId>>::new();
        for child in &call_ids {
            let Some(child_call) = self.tables.macro_calls.get(*child) else {
                self.expansions_partial = true;
                continue;
            };
            let Some(parent_expansion_identity) = child_call.parent_expansion_identity else {
                continue;
            };
            match self.call_ids_by_expansion_identity.get(&parent_expansion_identity).copied() {
                Some(parent) if parent != *child => {
                    children.entry(parent).or_default().push(*child);
                }
                Some(_) | None => {
                    self.expansions_partial = true;
                }
            }
        }
        for child_calls in children.values_mut() {
            child_calls.sort_by_key(|call| call.raw());
            child_calls.dedup();
        }
        children
    }

    fn recursive_emitted_tokens_for_call(
        &mut self,
        call: SourceMacroCallId,
        direct_tokens_by_call: &BTreeMap<SourceMacroCallId, Vec<SourceEmittedTokenId>>,
        child_calls_by_parent: &BTreeMap<SourceMacroCallId, Vec<SourceMacroCallId>>,
        visiting: &mut Vec<SourceMacroCallId>,
    ) -> Vec<SourceEmittedTokenId> {
        if visiting.contains(&call) {
            self.expansions_partial = true;
            return Vec::new();
        }

        visiting.push(call);
        let mut tokens = direct_tokens_by_call.get(&call).cloned().unwrap_or_default();
        if let Some(children) = child_calls_by_parent.get(&call) {
            for child in children {
                tokens.extend(self.recursive_emitted_tokens_for_call(
                    *child,
                    direct_tokens_by_call,
                    child_calls_by_parent,
                    visiting,
                ));
            }
        }
        visiting.pop();
        tokens.sort_by_key(|token| token.raw());
        tokens.dedup();
        tokens
    }

    fn mark_all_calls_unavailable(&mut self, reason: SourcePreprocUnavailable) {
        let call_ids = self.tables.macro_calls.iter().map(|call| call.id).collect::<Vec<_>>();
        for call in call_ids {
            self.mark_call_unavailable(call, reason.clone());
        }
    }

    fn mark_call_unavailable(&mut self, call: SourceMacroCallId, reason: SourcePreprocUnavailable) {
        self.expansions_partial = true;
        if let Some(call) = self.tables.macro_calls.get_mut(call) {
            call.expansion = None;
            call.status = SourceMacroCallStatus::ExpansionUnavailable(reason);
        }
    }

    fn source_is_predefine(&self, source: PreprocSourceId) -> bool {
        self.index.sources.iter().any(|candidate| {
            candidate.id == source && candidate.origin == PreprocSourceOrigin::Predefine
        })
    }

    fn unavailable_token_provenance(
        &mut self,
        reason: SourcePreprocUnavailable,
    ) -> SourceTokenProvenance {
        self.token_provenance_partial = true;
        SourceTokenProvenance::Unavailable(reason)
    }

    fn resolve_visible_reference(&mut self, name: &str) -> SourceMacroResolution {
        let Some(definition) = self.current_state.get(name).copied() else {
            return SourceMacroResolution::Undefined;
        };
        self.resolve_definition(definition, SourceMacroResolutionReason::VisibleDefinition)
    }

    fn resolve_visible_reference_at_position(
        &mut self,
        name: &str,
        position: SourcePosition,
    ) -> SourceMacroResolution {
        let Some(definition) = self
            .tables
            .state_timeline
            .state_at_position(position)
            .and_then(|state| state.definitions.get(name).copied())
        else {
            return SourceMacroResolution::Undefined;
        };
        self.resolve_definition(definition, SourceMacroResolutionReason::VisibleDefinition)
    }

    fn resolve_definition(
        &mut self,
        definition: SourceMacroDefinitionId,
        reason: SourceMacroResolutionReason,
    ) -> SourceMacroResolution {
        let definition_source = self
            .tables
            .macro_definitions
            .get(definition)
            .expect("definition id should point at inserted definition")
            .directive_range
            .source;
        match self.include_chain_for_source(definition_source) {
            Ok(include_chain) => {
                SourceMacroResolution::Resolved { definition, reason, include_chain }
            }
            Err(_) => {
                self.references_partial = true;
                if self.source_is_detached(definition_source) {
                    self.tables
                        .issues
                        .push(SourcePreprocFactIssue::DetachedSource { source: definition_source });
                    SourceMacroResolution::Unavailable(SourcePreprocUnavailable::DetachedSource {
                        source: definition_source,
                    })
                } else {
                    self.tables.issues.push(SourcePreprocFactIssue::IncludeChainUnavailable {
                        source: definition_source,
                    });
                    SourceMacroResolution::Unavailable(
                        SourcePreprocUnavailable::IncludeChainUnavailable {
                            source: definition_source,
                        },
                    )
                }
            }
        }
    }

    fn source_is_detached(&self, source: PreprocSourceId) -> bool {
        self.index.sources.iter().any(|candidate| {
            candidate.id == source && candidate.origin == PreprocSourceOrigin::Detached
        })
    }

    fn include_chain_for_source(
        &self,
        source: PreprocSourceId,
    ) -> Result<Vec<SourceIncludeChainEntry>, SourcePreprocError> {
        let mut chain = Vec::new();
        let mut current = source;
        let mut visited = BTreeMap::new();

        loop {
            if visited.insert(current, ()).is_some() {
                return Err(SourcePreprocError::IncludeCycle { source: current.raw() });
            }

            let Some(source) = self.index.sources.iter().find(|candidate| candidate.id == current)
            else {
                return Err(SourcePreprocError::MissingIncludedSource {
                    include_event_id: 0,
                    source: current.raw(),
                });
            };

            match source.origin {
                PreprocSourceOrigin::Root | PreprocSourceOrigin::Predefine => break,
                PreprocSourceOrigin::Detached => {
                    return Err(SourcePreprocError::MissingIncludeEdge { source: current.raw() });
                }
                PreprocSourceOrigin::Included { .. } => {
                    let edge = self
                        .tables
                        .include_graph
                        .edges()
                        .iter()
                        .find(|edge| edge.included_source == current)
                        .ok_or(SourcePreprocError::MissingIncludeEdge { source: current.raw() })?;
                    let directive = self
                        .tables
                        .include_graph
                        .directives()
                        .iter()
                        .find(|directive| directive.event_id == edge.include_event_id)
                        .ok_or(SourcePreprocError::MissingIncludeEvent {
                            include_event_id: edge.include_event_id.raw(),
                        })?;
                    chain.push(SourceIncludeChainEntry {
                        include_event_id: edge.include_event_id,
                        include_range: directive.directive_range,
                        included_source: current,
                    });
                    current = directive.directive_range.source;
                }
            }
        }

        chain.reverse();
        Ok(chain)
    }

    fn include_guard_definition_after_ifndef(
        &self,
        conditional_index: usize,
        name: &str,
    ) -> Option<SourceMacroDefinitionId> {
        let conditional = self.index.conditionals.get(conditional_index)?;
        if conditional.kind != MacroConditionalKind::IfNDef {
            return None;
        }

        let source = conditional.range.source;
        let (conditional_order, _) =
            self.index.event_records.iter().enumerate().find(|(_, directive)| {
                directive.kind == MacroEventKind::Conditional
                    && directive.index == conditional_index
            })?;
        for directive in self.index.event_records.iter().skip(conditional_order + 1) {
            if directive.range.source != source {
                continue;
            }
            match directive.kind {
                MacroEventKind::Define => {
                    let define = self.index.defines.get(directive.index)?;
                    if define.name.as_deref() == Some(name) {
                        return self.definition_ids_by_define_index.get(&directive.index).copied();
                    }
                }
                MacroEventKind::Branch => break,
                MacroEventKind::Undef
                | MacroEventKind::Include
                | MacroEventKind::Conditional
                | MacroEventKind::Usage => {}
            }
        }
        None
    }

    fn record_missing_reference_name(&mut self, event_id: SourcePreprocEventId) {
        self.references_partial = true;
        self.tables.issues.push(SourcePreprocFactIssue::MissingReferenceName { event_id });
    }

    fn record_missing_reference_name_range(&mut self, event_id: SourcePreprocEventId) {
        self.references_partial = true;
        self.tables.issues.push(SourcePreprocFactIssue::MissingReferenceNameRange { event_id });
    }

    fn record_state_checkpoint(&mut self, source_order: usize, boundary: SourcePosition) {
        let id = SourceMacroStateId::new(self.tables.state_timeline.states.len());
        self.tables
            .state_timeline
            .states
            .push(SourceMacroState { id, definitions: self.current_state.clone() });
        self.tables.state_timeline.checkpoints.push(SourceMacroStateCheckpoint {
            source_order,
            boundary,
            state: id,
        });
    }
}

impl SourcePosition {
    fn from_first_event(index: &SourcePreprocIndex) -> Self {
        index
            .event_records
            .first()
            .map(|record| SourcePosition {
                source: record.range.source,
                offset: record.range.range.start(),
            })
            .unwrap_or(SourcePosition {
                source: index.root_source.unwrap_or_else(|| PreprocSourceId::new(0)),
                offset: 0.into(),
            })
    }
}

fn boundary_after(directive_range: SourceRange) -> SourcePosition {
    SourcePosition { source: directive_range.source, offset: directive_range.range.end() }
}

fn partial_status(is_partial: bool) -> CapabilityStatus {
    if is_partial { CapabilityStatus::Partial } else { CapabilityStatus::Complete }
}

fn macro_reference_name_from_body_token(token: &SourceMacroToken) -> Option<SmolStr> {
    if !token.raw.starts_with('`') {
        return None;
    }
    let name = token.value.strip_prefix('`').unwrap_or(token.value.as_str());
    (!name.is_empty()).then(|| SmolStr::new(name))
}

fn emitted_token_range_from_ids(
    tokens: &[SourceEmittedTokenId],
) -> Option<SourceEmittedTokenRange> {
    let first = *tokens.first()?;
    let last = *tokens.last()?;
    let len = last.raw().checked_sub(first.raw())? + 1;
    (len == tokens.len()).then_some(SourceEmittedTokenRange { start: first, len })
}

fn merge_source_ranges(existing: Option<SourceRange>, next: SourceRange) -> Option<SourceRange> {
    let Some(existing) = existing else {
        return Some(next);
    };
    if existing.source != next.source {
        return Some(existing);
    }
    Some(SourceRange {
        source: existing.source,
        range: utils::line_index::TextRange::new(
            existing.range.start().min(next.range.start()),
            existing.range.end().max(next.range.end()),
        ),
    })
}
