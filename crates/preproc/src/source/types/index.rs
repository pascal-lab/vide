use super::*;
use crate::source::provenance::SourcePreprocTables;

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
    pub(in crate::source) tables: SourcePreprocTables,
}
