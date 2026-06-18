#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocSource {
    pub id: super::PreprocSourceId,
    pub path: smol_str::SmolStr,
    pub origin: PreprocSourceOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreprocSourceOrigin {
    Root,
    Included { include_event_id: super::SourcePreprocEventId },
    Predefine,
    Detached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIncludeEdge {
    pub include_event_id: super::SourcePreprocEventId,
    pub included_source: super::PreprocSourceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceIncludeChainEntry {
    pub include_event_id: super::SourcePreprocEventId,
    pub include_range: super::SourceRange,
    pub included_source: super::PreprocSourceId,
}
