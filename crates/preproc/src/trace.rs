use smol_str::SmolStr;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::macro_db::{MacroDefId, MacroName, MacroProfileId, MacroUseId, PredefineSource};

pub const PREPROC_TRACE_CAPABILITY: &str = "preproc_trace";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IncludeEventId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConditionalEventId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExpansionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExpandedTokenId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocTrace {
    pub profile: MacroProfileId,
    pub roots: Vec<FileId>,
    pub files: Vec<FilePreprocTrace>,
    pub include_events: Vec<IncludeEvent>,
    pub conditional_events: Vec<ConditionalEvent>,
    pub expansion_events: Vec<MacroExpansionEvent>,
    pub expanded_tokens: Vec<ExpandedToken>,
}

impl PreprocTrace {
    pub fn expansion_for_use(&self, use_id: MacroUseId) -> Option<&MacroExpansionEvent> {
        self.expansion_events.iter().find(|event| event.call.use_id == use_id)
    }

    pub fn origin_for_expanded_token(&self, token_id: ExpandedTokenId) -> ExpandedTokenOrigin {
        self.expanded_tokens
            .iter()
            .find(|token| token.id == token_id)
            .map(|token| ExpandedTokenOrigin::Origin(token.provenance.clone()))
            .unwrap_or(ExpandedTokenOrigin::Unknown { token_id })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePreprocTrace {
    pub file_id: FileId,
    pub include_stack: Vec<IncludeEventId>,
    pub include_events: Vec<IncludeEventId>,
    pub conditional_events: Vec<ConditionalEventId>,
    pub expansion_events: Vec<ExpansionId>,
    pub expanded_tokens: Vec<ExpandedTokenId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEvent {
    pub id: IncludeEventId,
    pub directive: IncludeDirective,
    pub target: IncludeTarget,
    pub included_file: Option<FileId>,
    pub parent: Option<IncludeEventId>,
    pub stack: Vec<IncludeEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeTarget {
    Literal { path: SmolStr, raw: SmolStr },
    MacroExpanded { raw: SmolStr, provenance: SourceProvenance },
    Unresolved { raw: SmolStr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalKind {
    IfDef,
    IfNDef,
    If,
    ElsIf,
    Else,
    EndIf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionalEvaluation {
    Taken,
    NotTaken,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalEvent {
    pub id: ConditionalEventId,
    pub kind: ConditionalKind,
    pub directive: SourceProvenance,
    pub expression_tokens: Vec<ConditionalToken>,
    pub evaluation: ConditionalEvaluation,
    pub include_stack: Vec<IncludeEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalToken {
    pub text: SmolStr,
    pub provenance: SourceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroExpansionEvent {
    pub id: ExpansionId,
    pub call: MacroCall,
    pub definition: MacroDefId,
    pub body: MacroBody,
    pub arguments: Vec<MacroArgument>,
    pub output_tokens: Vec<ExpandedTokenId>,
    pub include_stack: Vec<IncludeEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedToken {
    pub id: ExpandedTokenId,
    pub text: SmolStr,
    pub kind_hint: Option<SmolStr>,
    pub expansion: ExpansionId,
    pub provenance: SourceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandedTokenOrigin {
    Origin(SourceProvenance),
    Unknown { token_id: ExpandedTokenId },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceProvenance {
    File { file_id: FileId, range: TextRange },
    MacroCall(MacroCall),
    MacroArgument(MacroArgument),
    MacroBody(MacroBody),
    IncludeDirective(IncludeDirective),
    Virtual(VirtualSource),
    Unsupported { reason: SmolStr },
}

impl SourceProvenance {
    pub fn is_file_backed(&self) -> bool {
        matches!(
            self,
            SourceProvenance::File { .. }
                | SourceProvenance::MacroCall(_)
                | SourceProvenance::MacroArgument(_)
                | SourceProvenance::MacroBody(_)
                | SourceProvenance::IncludeDirective(_)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroCall {
    pub use_id: MacroUseId,
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroArgument {
    pub call: MacroCall,
    pub index: u32,
    pub name: Option<MacroName>,
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroBody {
    pub definition: MacroDefId,
    pub file_id: Option<FileId>,
    pub range: Option<TextRange>,
    pub token_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDirective {
    pub event: Option<IncludeEventId>,
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualSource {
    Predefine {
        profile: MacroProfileId,
        name: MacroName,
        value: Option<SmolStr>,
        source: PredefineSource,
    },
    Generated {
        reason: SmolStr,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceCapability {
    Available,
    CapabilityUnavailable(CapabilityUnavailable),
}

impl TraceCapability {
    pub fn missing_preproc_trace() -> Self {
        Self::CapabilityUnavailable(CapabilityUnavailable::missing_preproc_trace())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocTraceResult<T> {
    Available(T),
    CapabilityUnavailable(CapabilityUnavailable),
}

impl<T> PreprocTraceResult<T> {
    pub fn missing_preproc_trace() -> Self {
        Self::CapabilityUnavailable(CapabilityUnavailable::missing_preproc_trace())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityUnavailable {
    pub capability: SmolStr,
    pub reason: TraceUnavailableReason,
}

impl CapabilityUnavailable {
    pub fn missing_preproc_trace() -> Self {
        Self {
            capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
            reason: TraceUnavailableReason::MissingPreprocTrace,
        }
    }

    pub fn binding_unavailable(reason: impl Into<SmolStr>) -> Self {
        Self {
            capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
            reason: TraceUnavailableReason::BindingUnavailable { reason: reason.into() },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceUnavailableReason {
    MissingPreprocTrace,
    BindingUnavailable { reason: SmolStr },
}

#[cfg(test)]
mod tests {
    use utils::line_index::TextSize;

    use super::*;

    fn range(start: u32, end: u32) -> TextRange {
        TextRange::new(TextSize::from(start), TextSize::from(end))
    }

    #[test]
    fn virtual_predefine_provenance_does_not_invent_file_range() {
        let provenance = SourceProvenance::Virtual(VirtualSource::Predefine {
            profile: MacroProfileId(1),
            name: MacroName::new("WIDTH"),
            value: Some(SmolStr::new("32")),
            source: PredefineSource::CommandLine,
        });

        assert!(!provenance.is_file_backed());
        assert_eq!(
            provenance,
            SourceProvenance::Virtual(VirtualSource::Predefine {
                profile: MacroProfileId(1),
                name: MacroName::new("WIDTH"),
                value: Some(SmolStr::new("32")),
                source: PredefineSource::CommandLine,
            })
        );
    }

    #[test]
    fn macro_body_argument_and_callsite_provenance_are_distinct() {
        let call = MacroCall { use_id: MacroUseId(0), file_id: FileId(1), range: range(30, 44) };
        let argument = MacroArgument {
            call: call.clone(),
            index: 0,
            name: Some(MacroName::new("name")),
            file_id: FileId(1),
            range: range(40, 43),
        };
        let body = MacroBody {
            definition: MacroDefId(7),
            file_id: Some(FileId(0)),
            range: Some(range(18, 28)),
            token_index: Some(1),
        };
        let trace = PreprocTrace {
            profile: MacroProfileId(1),
            roots: vec![FileId(1)],
            files: Vec::new(),
            include_events: Vec::new(),
            conditional_events: Vec::new(),
            expansion_events: vec![MacroExpansionEvent {
                id: ExpansionId(0),
                call: call.clone(),
                definition: MacroDefId(7),
                body: body.clone(),
                arguments: vec![argument.clone()],
                output_tokens: vec![ExpandedTokenId(0), ExpandedTokenId(1)],
                include_stack: Vec::new(),
            }],
            expanded_tokens: vec![
                ExpandedToken {
                    id: ExpandedTokenId(0),
                    text: SmolStr::new("logic"),
                    kind_hint: None,
                    expansion: ExpansionId(0),
                    provenance: SourceProvenance::MacroBody(body.clone()),
                },
                ExpandedToken {
                    id: ExpandedTokenId(1),
                    text: SmolStr::new("foo"),
                    kind_hint: None,
                    expansion: ExpansionId(0),
                    provenance: SourceProvenance::MacroArgument(argument.clone()),
                },
            ],
        };

        assert_eq!(trace.expansion_for_use(MacroUseId(0)).unwrap().call, call);
        assert_eq!(
            trace.origin_for_expanded_token(ExpandedTokenId(0)),
            ExpandedTokenOrigin::Origin(SourceProvenance::MacroBody(body))
        );
        assert_eq!(
            trace.origin_for_expanded_token(ExpandedTokenId(1)),
            ExpandedTokenOrigin::Origin(SourceProvenance::MacroArgument(argument))
        );
    }

    #[test]
    fn include_stack_provenance_can_be_represented() {
        let root_include = IncludeEvent {
            id: IncludeEventId(0),
            directive: IncludeDirective {
                event: Some(IncludeEventId(0)),
                file_id: FileId(0),
                range: range(0, 19),
            },
            target: IncludeTarget::Literal {
                path: SmolStr::new("defs.svh"),
                raw: SmolStr::new("\"defs.svh\""),
            },
            included_file: Some(FileId(1)),
            parent: None,
            stack: Vec::new(),
        };
        let nested_include = IncludeEvent {
            id: IncludeEventId(1),
            directive: IncludeDirective {
                event: Some(IncludeEventId(1)),
                file_id: FileId(1),
                range: range(0, 20),
            },
            target: IncludeTarget::Literal {
                path: SmolStr::new("more.svh"),
                raw: SmolStr::new("\"more.svh\""),
            },
            included_file: Some(FileId(2)),
            parent: Some(IncludeEventId(0)),
            stack: vec![IncludeEventId(0)],
        };
        let trace = PreprocTrace {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: vec![FilePreprocTrace {
                file_id: FileId(2),
                include_stack: vec![IncludeEventId(0), IncludeEventId(1)],
                include_events: Vec::new(),
                conditional_events: Vec::new(),
                expansion_events: Vec::new(),
                expanded_tokens: Vec::new(),
            }],
            include_events: vec![root_include, nested_include],
            conditional_events: Vec::new(),
            expansion_events: Vec::new(),
            expanded_tokens: Vec::new(),
        };

        assert_eq!(trace.files[0].include_stack, vec![IncludeEventId(0), IncludeEventId(1)]);
        assert_eq!(trace.include_events[1].parent, Some(IncludeEventId(0)));
    }

    #[test]
    fn unknown_expanded_token_is_not_an_empty_trace() {
        let trace = PreprocTrace {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            files: Vec::new(),
            include_events: Vec::new(),
            conditional_events: Vec::new(),
            expansion_events: Vec::new(),
            expanded_tokens: Vec::new(),
        };

        assert_eq!(
            trace.origin_for_expanded_token(ExpandedTokenId(99)),
            ExpandedTokenOrigin::Unknown { token_id: ExpandedTokenId(99) }
        );
    }

    #[test]
    fn missing_trace_is_explicitly_unavailable() {
        assert_eq!(
            TraceCapability::missing_preproc_trace(),
            TraceCapability::CapabilityUnavailable(CapabilityUnavailable {
                capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
                reason: TraceUnavailableReason::MissingPreprocTrace,
            })
        );
        assert_eq!(
            PreprocTraceResult::<PreprocTrace>::missing_preproc_trace(),
            PreprocTraceResult::CapabilityUnavailable(CapabilityUnavailable {
                capability: SmolStr::new(PREPROC_TRACE_CAPABILITY),
                reason: TraceUnavailableReason::MissingPreprocTrace,
            })
        );
    }
}
