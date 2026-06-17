use super::*;

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
