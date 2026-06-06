use std::collections::BTreeMap;

use smol_str::SmolStr;
use utils::line_index::TextSize;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroDefinition {
    pub id: SourceMacroDefinitionId,
    pub event_id: SourcePreprocEventId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SourceMacroCallSignature {
    source: PreprocSourceId,
    start: TextSize,
    end: TextSize,
    name: SmolStr,
}

impl SourceMacroCallSignature {
    fn new(name: SmolStr, range: SourceRange) -> Self {
        Self { source: range.source, start: range.range.start(), end: range.range.end(), name }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMacroCall {
    pub id: SourceMacroCallId,
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
        definition: SourceMacroDefinitionId,
        body_token_range: SourceRange,
        call: SourceMacroCallId,
    },
    MacroArgument {
        call: SourceMacroCallId,
        argument_index: usize,
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
    MacroCallAuthorityUnavailable,
    EmittedTokenAuthorityUnavailable,
    TokenProvenanceAuthorityUnavailable,
    ExpansionAuthorityUnavailable,
    MissingEmittedTokenMacroCall { source: PreprocSourceId },
    MissingEmittedTokenMacroDefinition { call: SourceMacroCallId },
    MissingEmittedTokenMacroBody { call: SourceMacroCallId },
    MissingEmittedTokenMacroArgument { call: SourceMacroCallId },
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

impl Default for SourceIncludeGraph {
    fn default() -> Self {
        Self { directives: Vec::new(), edges: Vec::new() }
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

impl Default for SourceMacroStateTimeline {
    fn default() -> Self {
        Self {
            states: Vec::new(),
            checkpoints: Vec::new(),
            source_order_boundaries: BTreeMap::new(),
            final_source_order: 0,
        }
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
    call_ids_by_signature: BTreeMap<SourceMacroCallSignature, SourceMacroCallId>,
    current_state: BTreeMap<SmolStr, SourceMacroDefinitionId>,
    definition_ranges_partial: bool,
    include_edges_partial: bool,
    references_partial: bool,
    token_provenance_partial: bool,
}

impl<'a> SourcePreprocModelBuilder<'a> {
    pub fn new(index: &'a SourcePreprocIndex) -> Self {
        Self {
            index,
            tables: SourcePreprocTables::default(),
            definition_ids_by_define_index: BTreeMap::new(),
            call_ids_by_signature: BTreeMap::new(),
            current_state: BTreeMap::new(),
            definition_ranges_partial: false,
            include_edges_partial: false,
            references_partial: false,
            token_provenance_partial: false,
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
        self.tables.capabilities = SourcePreprocCapabilities {
            source_events: CapabilityStatus::Complete,
            definition_name_ranges: partial_status(self.definition_ranges_partial),
            include_edges: partial_status(self.include_edges_partial),
            inactive_ranges: CapabilityStatus::Complete,
            macro_reference_resolution: partial_status(self.references_partial),
            macro_calls: partial_status(self.references_partial),
            macro_expansions: CapabilityStatus::Unavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
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
                name,
                name_range,
                directive_range: define.range,
                params: define.params.clone(),
                body_tokens: define.body.clone(),
            });
            self.definition_ids_by_define_index.insert(define_index, id);
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
        self.push_call(reference, name, directive_range, resolution);
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
        name: SmolStr,
        call_range: SourceRange,
        callee: SourceMacroResolution,
    ) {
        let id = SourceMacroCallId::new(self.tables.macro_calls.len());
        self.tables.macro_calls.push(SourceMacroCall {
            id,
            reference,
            call_range,
            callee,
            arguments: Vec::new(),
            expansion: None,
            status: SourceMacroCallStatus::ExpansionUnavailable(
                SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
            ),
        });
        self.call_ids_by_signature.insert(SourceMacroCallSignature::new(name, call_range), id);
    }

    fn build_emitted_token_tables(&mut self) {
        for index in 0..self.index.emitted_tokens.len() {
            let token = self.index.emitted_tokens[index].clone();
            let provenance = self.resolve_emitted_token_provenance(&token);
            let provenance_id = SourceTokenProvenanceId::new(self.tables.token_provenance.len());
            self.tables.token_provenance.push(provenance);

            let token_id = SourceEmittedTokenId::new(self.tables.emitted_tokens.len());
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
        token: &SourceEmittedTokenFact,
    ) -> SourceTokenProvenance {
        match &token.provenance {
            SourceTokenProvenanceFact::Source { token_range } => {
                SourceTokenProvenance::Source { token_range: *token_range }
            }
            SourceTokenProvenanceFact::MacroBody { macro_name, call_range, body_token_range } => {
                self.resolve_macro_body_token_provenance(
                    token,
                    macro_name.clone(),
                    *call_range,
                    *body_token_range,
                )
            }
            SourceTokenProvenanceFact::MacroArgument {
                macro_name,
                call_range,
                body_token_range,
                argument_token_range,
            } => self.resolve_macro_argument_token_provenance(
                macro_name.clone(),
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
        token: &SourceEmittedTokenFact,
        macro_name: SmolStr,
        call_range: SourceRange,
        body_token_range: SourceRange,
    ) -> SourceTokenProvenance {
        if self.source_is_predefine(body_token_range.source) {
            return SourceTokenProvenance::Predefine { source: body_token_range.source };
        }

        let Ok(call) = self.call_for_emitted_token(macro_name, call_range) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroCall {
                    source: call_range.source,
                },
            );
        };
        let Ok(definition) = self.definition_for_call(call) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroDefinition { call },
            );
        };

        if !self.definition_body_contains_raw_token(
            definition,
            body_token_range,
            token.raw.as_str(),
        ) {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::UnsupportedEmittedTokenProvenance,
            );
        }

        SourceTokenProvenance::MacroBody { definition, body_token_range, call }
    }

    fn resolve_macro_argument_token_provenance(
        &mut self,
        macro_name: SmolStr,
        call_range: SourceRange,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    ) -> SourceTokenProvenance {
        let Ok(call) = self.call_for_emitted_token(macro_name, call_range) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroCall {
                    source: call_range.source,
                },
            );
        };
        let Ok(argument_index) = self.argument_index_for_body_token(call, body_token_range) else {
            return self.unavailable_token_provenance(
                SourcePreprocUnavailable::MissingEmittedTokenMacroArgument { call },
            );
        };

        SourceTokenProvenance::MacroArgument { call, argument_index, argument_token_range }
    }

    fn call_for_emitted_token(
        &self,
        macro_name: SmolStr,
        call_range: SourceRange,
    ) -> Result<SourceMacroCallId, ()> {
        self.call_ids_by_signature
            .get(&SourceMacroCallSignature::new(macro_name, call_range))
            .copied()
            .ok_or(())
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

    fn definition_body_contains_raw_token(
        &self,
        definition: SourceMacroDefinitionId,
        body_token_range: SourceRange,
        raw: &str,
    ) -> bool {
        let Some(definition) = self.tables.macro_definitions.get(definition) else {
            return false;
        };
        definition
            .body_tokens
            .iter()
            .any(|token| token.range == Some(body_token_range) && token.raw.as_str() == raw)
    }

    fn argument_index_for_body_token(
        &self,
        call: SourceMacroCallId,
        body_token_range: SourceRange,
    ) -> Result<usize, ()> {
        let definition = self.definition_for_call(call)?;
        let definition = self.tables.macro_definitions.get(definition).ok_or(())?;
        let body_token = definition
            .body_tokens
            .iter()
            .find(|token| token.range == Some(body_token_range))
            .ok_or(())?;
        let params = definition.params.as_ref().ok_or(())?;
        params.iter().position(|param| param.name.as_ref() == Some(&body_token.value)).ok_or(())
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
