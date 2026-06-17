use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::*;

pub(in crate::source) struct SourcePreprocModelBuilder {
    model: SourcePreprocModel,
    definition_ids_by_define_index: BTreeMap<usize, SourceMacroDefinitionId>,
    definition_ids_by_identity: BTreeMap<MacroDefinitionId, SourceMacroDefinitionId>,
    call_ids_by_identity: BTreeMap<MacroCallId, SourceMacroCallId>,
    call_ids_by_expansion_identity: BTreeMap<MacroExpansionId, SourceMacroCallId>,
    // Expansion ownership comes from trace identities, not from source provenance.
    emitted_token_owners: BTreeMap<SourceEmittedTokenId, SourceMacroCallId>,
    current_state: BTreeMap<SmolStr, SourceMacroDefinitionId>,
    definition_ranges_partial: bool,
    include_edges_partial: bool,
    references_partial: bool,
    macro_calls_partial: bool,
    expansions_partial: bool,
}

mod definitions;
mod emitted;
mod emitted_helpers;
mod expansion_helpers;
mod expansions;
mod references;
mod resolution;
mod state;
mod token_origin;

impl SourcePreprocModelBuilder {
    pub(in crate::source) fn new(index: SourcePreprocIndex) -> Self {
        Self {
            model: SourcePreprocModel {
                index,
                macro_definitions: SourceMacroDefinitionTable::default(),
                macro_references: SourceMacroReferenceTable::default(),
                macro_calls: SourceMacroCallTable::default(),
                macro_expansions: SourceMacroExpansionTable::default(),
                emitted_tokens: SourceEmittedTokenTable::default(),
                token_origins: SourceTokenOriginTable::default(),
                include_graph: SourceIncludeGraph::default(),
                inactive_ranges: Vec::new(),
                state_timeline: SourceMacroStateTimeline::default(),
                issues: Vec::new(),
            },
            definition_ids_by_define_index: BTreeMap::new(),
            definition_ids_by_identity: BTreeMap::new(),
            call_ids_by_identity: BTreeMap::new(),
            call_ids_by_expansion_identity: BTreeMap::new(),
            emitted_token_owners: BTreeMap::new(),
            current_state: BTreeMap::new(),
            definition_ranges_partial: false,
            include_edges_partial: false,
            references_partial: false,
            macro_calls_partial: false,
            expansions_partial: false,
        }
    }

    pub(in crate::source) fn build(mut self) -> SourcePreprocModel {
        self.build_tables();
        self.model
    }

    fn build_tables(&mut self) {
        self.build_definition_table();
        self.build_include_graph();
        self.record_position_boundaries();
        self.record_state_checkpoint(0, SourcePosition::from_first_event(&self.model.index));
        self.scan_references_and_state();
        self.build_emitted_token_tables();
        self.build_macro_expansion_graph();
        self.record_macro_body_references_for_calls();
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

pub(in crate::source::provenance::builder) fn boundary_after(
    directive_range: SourceRange,
) -> SourcePosition {
    SourcePosition { source: directive_range.source, offset: directive_range.range.end() }
}
