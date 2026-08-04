use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::*;

pub(in crate::source) struct SourcePreprocModelBuilder {
    model: SourcePreprocModel,
    /// Raw event projections collected from the trace. Consumed only while
    /// deriving the resolved tables; they do not survive into the model.
    event_records: Vec<SourcePreprocEventRecord>,
    defines: Vec<SourceMacroDefine>,
    undefs: Vec<SourceMacroUndef>,
    includes: Vec<SourceMacroInclude>,
    conditionals: Vec<SourceMacroConditional>,
    usages: Vec<SourceMacroUsage>,
    include_edges: Vec<SourceIncludeEdge>,
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
mod trace;

impl SourcePreprocModelBuilder {
    pub(in crate::source) fn build(mut self) -> SourcePreprocModel {
        self.build_tables();
        self.model
    }

    fn build_tables(&mut self) {
        self.build_definition_table();
        self.build_include_graph();
        self.record_position_boundaries();
        self.record_state_checkpoint(0, SourcePosition::from_first_event(&self));
        self.scan_references_and_state();
        self.record_macro_body_references_for_calls();
    }
}

impl SourcePosition {
    fn from_first_event(builder: &SourcePreprocModelBuilder) -> Self {
        builder
            .event_records
            .first()
            .map(|record| SourcePosition {
                source: record.range.source,
                offset: record.range.range.start(),
            })
            .unwrap_or(SourcePosition {
                source: builder.model.root_source,
                offset: 0.into(),
            })
    }
}

pub(in crate::source::tables::builder) fn boundary_after(
    directive_range: SourceRange,
) -> SourcePosition {
    SourcePosition { source: directive_range.source, offset: directive_range.range.end() }
}
