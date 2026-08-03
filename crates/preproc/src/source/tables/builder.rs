use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::*;

pub(in crate::source) struct SourcePreprocModelBuilder {
    model: SourcePreprocModel,
    definition_ids_by_define_index: BTreeMap<usize, SourceMacroDefinitionId>,
    definitions_by_trace_id: BTreeMap<MacroDefinitionId, SourceMacroDefinitionId>,
    calls_by_trace_id: BTreeMap<MacroCallId, SourceMacroCallId>,
    current_state: BTreeMap<SmolStr, SourceMacroDefinitionId>,
}

mod body_references;
mod definitions;
mod references;
mod resolution;
mod state;

impl SourcePreprocModelBuilder {
    pub(in crate::source) fn new(mut model: SourcePreprocModel) -> Self {
        model.macro_definitions = SourceMacroDefinitionTable::default();
        model.macro_references = SourceMacroReferenceTable::default();
        model.macro_calls = SourceMacroCallTable::default();
        model.include_graph = SourceIncludeGraph::default();
        model.state_timeline = SourceMacroStateTimeline::default();
        Self {
            model,
            definition_ids_by_define_index: BTreeMap::new(),
            definitions_by_trace_id: BTreeMap::new(),
            calls_by_trace_id: BTreeMap::new(),
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
