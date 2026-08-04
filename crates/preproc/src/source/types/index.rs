use super::*;
use crate::source::tables::{
    SourceIncludeGraph, SourceMacroCallTable, SourceMacroDefinitionTable,
    SourceMacroReferenceTable, SourceMacroStateTimeline,
};

/// The source preprocessing model for one parsed file.
///
/// Built in a single pass from the slang preprocessor `Trace`. The raw event
/// projections (event records, defines, undefs, includes, conditionals,
/// usages) are private builder inputs consumed while deriving the resolved
/// tables below; queries go through those tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocModel {
    pub(in crate::source) root_source: PreprocSourceId,
    pub(in crate::source) sources: Vec<PreprocSource>,
    pub(in crate::source) inactive_ranges: Vec<SourceRange>,
    pub(in crate::source) macro_definitions: SourceMacroDefinitionTable,
    pub(in crate::source) macro_references: SourceMacroReferenceTable,
    pub(in crate::source) macro_calls: SourceMacroCallTable,
    pub(in crate::source) include_graph: SourceIncludeGraph,
    pub(in crate::source) state_timeline: SourceMacroStateTimeline,
}
