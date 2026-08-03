use super::*;
use crate::source::tables::{
    SourceIncludeGraph, SourceMacroCallTable, SourceMacroDefinitionTable, SourceMacroReferenceTable,
    SourceMacroStateTimeline,
};

/// The source preprocessing model for one parsed file.
///
/// Built in a single pass from the slang preprocessor `Trace`. The raw event
/// projections (sources, event records, emitted token records) are private
/// builder inputs; queries go through the resolved tables below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocModel {
    pub(in crate::source) root_source: PreprocSourceId,
    pub(in crate::source) sources: Vec<PreprocSource>,
    pub(in crate::source) include_edges: Vec<SourceIncludeEdge>,
    pub(in crate::source) event_records: Vec<SourcePreprocEventRecord>,
    pub(in crate::source) defines: Vec<SourceMacroDefine>,
    pub(in crate::source) undefs: Vec<SourceMacroUndef>,
    pub(in crate::source) includes: Vec<SourceMacroInclude>,
    pub(in crate::source) conditionals: Vec<SourceMacroConditional>,
    pub(in crate::source) usages: Vec<SourceMacroUsage>,
    pub(in crate::source) inactive_ranges: Vec<SourceRange>,
    pub(in crate::source) macro_definitions: SourceMacroDefinitionTable,
    pub(in crate::source) macro_references: SourceMacroReferenceTable,
    pub(in crate::source) macro_calls: SourceMacroCallTable,
    pub(in crate::source) include_graph: SourceIncludeGraph,
    pub(in crate::source) state_timeline: SourceMacroStateTimeline,
}
