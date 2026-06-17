use super::*;

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

impl SourcePreprocTables {
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
        let source_end_order = self
            .source_order_scopes
            .get(&position.source)
            .map(|scope| scope.end_order)
            .unwrap_or(self.final_source_order);
        let Some(boundaries) = self.source_order_boundaries.get(&position.source) else {
            return source_end_order;
        };
        let index =
            boundaries.partition_point(|boundary| boundary.boundary.offset <= position.offset);
        boundaries.get(index).map(|boundary| boundary.source_order).unwrap_or(source_end_order)
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
