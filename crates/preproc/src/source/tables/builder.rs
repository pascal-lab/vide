use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::*;

pub(in crate::source) struct SourcePreprocModelBuilder {
    model: SourcePreprocModel,
    definition_ids_by_define_index: BTreeMap<usize, SourceMacroDefinitionId>,
    definitions_by_trace_id: BTreeMap<MacroDefinitionId, SourceMacroDefinitionId>,
    calls_by_trace_id: BTreeMap<MacroCallId, SourceMacroCallId>,
    calls_by_expansion_trace_id: BTreeMap<MacroExpansionId, SourceMacroCallId>,
    emitted_token_owners: BTreeMap<SourceEmittedTokenId, SourceMacroCallId>,
    current_state: BTreeMap<SmolStr, SourceMacroDefinitionId>,
}

mod definitions;
mod emitted;
mod emitted_helpers;
mod emitted_origins;
mod expansion_helpers;
mod expansions;
mod references;
mod resolution;
mod state;
mod token_origin;

impl SourcePreprocModelBuilder {
    pub(in crate::source) fn new(mut model: SourcePreprocModel) -> Self {
        model.macro_definitions = SourceMacroDefinitionTable::default();
        model.macro_references = SourceMacroReferenceTable::default();
        model.macro_calls = SourceMacroCallTable::default();
        model.macro_expansions = SourceMacroExpansionTable::default();
        model.emitted_tokens = SourceEmittedTokenTable::default();
        model.token_origins = SourceTokenOriginTable::default();
        model.include_graph = SourceIncludeGraph::default();
        model.state_timeline = SourceMacroStateTimeline::default();
        Self {
            model,
            definition_ids_by_define_index: BTreeMap::new(),
            definitions_by_trace_id: BTreeMap::new(),
            calls_by_trace_id: BTreeMap::new(),
            calls_by_expansion_trace_id: BTreeMap::new(),
            emitted_token_owners: BTreeMap::new(),
            current_state: BTreeMap::new(),
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
        self.record_state_checkpoint(0, SourcePosition::from_first_event(&self.model));
        self.scan_references_and_state();
        self.build_emitted_token_tables();
        self.build_macro_expansion_graph();
        self.record_macro_body_references_for_calls();
    }
}

impl SourcePosition {
    fn from_first_event(model: &SourcePreprocModel) -> Self {
        model
            .event_records
            .first()
            .map(|record| SourcePosition {
                source: record.range.source,
                offset: record.range.range.start(),
            })
            .unwrap_or(SourcePosition { source: model.root_source, offset: 0.into() })
    }
}

pub(in crate::source::tables::builder) fn boundary_after(
    directive_range: SourceRange,
) -> SourcePosition {
    SourcePosition { source: directive_range.source, offset: directive_range.range.end() }
}
