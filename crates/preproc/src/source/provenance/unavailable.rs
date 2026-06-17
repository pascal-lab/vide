use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocUnavailable {
    DetachedSource { source: PreprocSourceId },
    MissingPredefineSourceText { source: PreprocSourceId },
    UnverifiedPredefineSource { source: PreprocSourceId },
    ExpansionAuthorityUnavailable,
    MissingMacroCall { call: SourceMacroCallId },
    MissingMacroExpansion { call: SourceMacroCallId },
    UnknownMacroUsageDefinitionIdentity { identity: SourceMacroDefinitionKey },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocFactIssue {
    MissingDefinitionName { event_id: SourcePreprocEventId },
    MissingDefinitionNameRange { event_id: SourcePreprocEventId },
    MissingReferenceName { event_id: SourcePreprocEventId },
    MissingReferenceNameRange { event_id: SourcePreprocEventId },
    DetachedSource { source: PreprocSourceId },
}
