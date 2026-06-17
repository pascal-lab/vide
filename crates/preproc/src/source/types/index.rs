use super::*;
use crate::source::provenance::{
    SourceEmittedTokenTable, SourceIncludeGraph, SourceMacroCallTable, SourceMacroDefinitionTable,
    SourceMacroExpansionTable, SourceMacroReferenceTable, SourceMacroStateTimeline,
    SourcePreprocIssue, SourceTokenProvenanceTable,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocIndex {
    pub root_source: Option<PreprocSourceId>,
    pub sources: Vec<PreprocSource>,
    pub include_edges: Vec<SourceIncludeEdge>,
    pub event_records: Vec<SourcePreprocEventRecord>,
    pub emitted_tokens: Vec<SourceEmittedTokenRecord>,
    pub defines: Vec<SourceMacroDefine>,
    pub undefs: Vec<SourceMacroUndef>,
    pub includes: Vec<SourceMacroInclude>,
    pub conditionals: Vec<SourceMacroConditional>,
    pub usages: Vec<SourceMacroUsage>,
    pub inactive_ranges: Vec<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocModel {
    pub(in crate::source) index: SourcePreprocIndex,
    pub(in crate::source) macro_definitions: SourceMacroDefinitionTable,
    pub(in crate::source) macro_references: SourceMacroReferenceTable,
    pub(in crate::source) macro_calls: SourceMacroCallTable,
    pub(in crate::source) macro_expansions: SourceMacroExpansionTable,
    pub(in crate::source) emitted_tokens: SourceEmittedTokenTable,
    pub(in crate::source) token_provenance: SourceTokenProvenanceTable,
    pub(in crate::source) include_graph: SourceIncludeGraph,
    pub(in crate::source) inactive_ranges: Vec<SourceRange>,
    pub(in crate::source) state_timeline: SourceMacroStateTimeline,
    pub(in crate::source) issues: Vec<SourcePreprocIssue>,
}
