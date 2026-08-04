use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocUnavailable {
    DetachedSource { source: PreprocSourceId },
    MissingPredefineSourceText { source: PreprocSourceId },
    UnverifiedPredefineSource { source: PreprocSourceId },
    MissingMacroCall { call: SourceMacroCallId },
    MissingMacroExpansion { call: SourceMacroCallId },
    UnknownMacroUsageDefinition { definition: MacroDefinitionId },
}
