use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocUnavailable {
    MissingDefinitionName { event_id: SourcePreprocEventId },
    MissingDefinitionNameRange { event_id: SourcePreprocEventId },
    MissingReferenceName { event_id: SourcePreprocEventId },
    MissingReferenceNameRange { event_id: SourcePreprocEventId },
    DetachedSource { source: PreprocSourceId },
    MissingPredefineSourceText { source: PreprocSourceId },
    UnverifiedPredefineSource { source: PreprocSourceId },
    MacroCallAuthorityUnavailable,
    EmittedTokenAuthorityUnavailable,
    TokenProvenanceAuthorityUnavailable,
    ExpansionAuthorityUnavailable,
    MissingMacroCall { call: SourceMacroCallId },
    MissingMacroExpansion { call: SourceMacroCallId },
    MissingEmittedTokenMacroCall { source: PreprocSourceId },
    UnknownMacroUsageDefinitionIdentity { identity: SourceMacroDefinitionKey },
    MissingEmittedTokenMacroCallIdentity,
    UnknownEmittedTokenMacroCallIdentity { identity: SourceMacroCallKey },
    MissingEmittedTokenMacroDefinitionIdentity,
    UnknownEmittedTokenMacroDefinitionIdentity { identity: SourceMacroDefinitionKey },
    MissingEmittedTokenMacroExpansionIdentity { call: SourceMacroCallId },
    UnmappedParentMacroExpansionIdentity { identity: SourceMacroExpansionKey },
    MissingEmittedTokenMacroDefinition { call: SourceMacroCallId },
    MissingEmittedTokenMacroBody { call: SourceMacroCallId },
    MissingEmittedTokenMacroArgument { call: SourceMacroCallId },
    NonContiguousEmittedTokenRange { call: SourceMacroCallId },
    UnsupportedEmittedTokenProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocFactIssue {
    MissingDefinitionName { event_id: SourcePreprocEventId },
    MissingDefinitionNameRange { event_id: SourcePreprocEventId },
    MissingReferenceName { event_id: SourcePreprocEventId },
    MissingReferenceNameRange { event_id: SourcePreprocEventId },
    DetachedSource { source: PreprocSourceId },
}
