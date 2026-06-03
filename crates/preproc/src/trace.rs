use smol_str::SmolStr;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::macro_db::{MacroDefId, MacroName, MacroProfileId, MacroUseId, PredefineSource};

pub const PREPROC_TRACE_CAPABILITY: &str = "preproc_trace";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IncludeEventId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreprocFrameId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceInstanceId(pub u32);

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
    pub source_instances: Vec<SourceInstance>,
    pub frames: Vec<PreprocFrame>,
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
    pub source_instance: SourceInstanceId,
    pub frame: PreprocFrameId,
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
    pub including_frame: PreprocFrameId,
    pub included_frame: Option<PreprocFrameId>,
    pub parent: Option<IncludeEventId>,
    pub stack: Vec<IncludeEventId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInstance {
    pub id: SourceInstanceId,
    pub file_id: FileId,
    pub frame: PreprocFrameId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocFrame {
    pub id: PreprocFrameId,
    pub source_instance: SourceInstanceId,
    pub file_id: FileId,
    pub entered_by: Option<IncludeEventId>,
    pub parent: Option<PreprocFrameId>,
    pub include_stack: Vec<IncludeEventId>,
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
    pub fn primary_file_range(&self) -> Result<FileRange, ProvenanceUnavailable> {
        match self {
            SourceProvenance::File { file_id, range } => {
                Ok(FileRange { file_id: *file_id, range: *range })
            }
            SourceProvenance::MacroCall(call) => {
                Ok(FileRange { file_id: call.file_id, range: call.range })
            }
            SourceProvenance::MacroArgument(argument) => {
                Ok(FileRange { file_id: argument.file_id, range: argument.range })
            }
            SourceProvenance::MacroBody(body) => {
                macro_body_file_range(body).ok_or(ProvenanceUnavailable::MissingFileRange {
                    provenance: SmolStr::new("macro_body"),
                })
            }
            SourceProvenance::IncludeDirective(directive) => {
                Ok(FileRange { file_id: directive.file_id, range: directive.range })
            }
            SourceProvenance::Virtual(source) => {
                Err(ProvenanceUnavailable::Virtual { source: source.clone() })
            }
            SourceProvenance::Unsupported { reason } => {
                Err(ProvenanceUnavailable::Unsupported { reason: reason.clone() })
            }
        }
    }

    pub fn editable_file_range(&self) -> Result<FileRange, ProvenanceUnavailable> {
        match self {
            SourceProvenance::MacroBody(_) => Err(ProvenanceUnavailable::NotEditable {
                reason: SmolStr::new("macro body provenance is not editable-safe"),
            }),
            _ => self.primary_file_range(),
        }
    }

    pub fn related_locations(&self) -> Vec<SourceProvenance> {
        match self {
            SourceProvenance::MacroArgument(argument) => {
                vec![SourceProvenance::MacroCall(argument.call.clone())]
            }
            SourceProvenance::MacroBody(body) => macro_body_file_range(body)
                .map(|range| {
                    vec![SourceProvenance::File { file_id: range.file_id, range: range.range }]
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn is_file_backed(&self) -> bool {
        self.primary_file_range().is_ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileRange {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvenanceUnavailable {
    MissingFileRange { provenance: SmolStr },
    NotEditable { reason: SmolStr },
    Virtual { source: VirtualSource },
    Unsupported { reason: SmolStr },
}

fn macro_body_file_range(body: &MacroBody) -> Option<FileRange> {
    Some(FileRange { file_id: body.file_id?, range: body.range? })
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
            source_instances: Vec::new(),
            frames: Vec::new(),
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
    fn macro_body_without_file_range_is_not_file_backed() {
        let body = MacroBody {
            definition: MacroDefId(7),
            file_id: None,
            range: None,
            token_index: Some(1),
        };

        let provenance = SourceProvenance::MacroBody(body);

        assert!(!provenance.is_file_backed());
        assert_eq!(
            provenance.primary_file_range(),
            Err(ProvenanceUnavailable::MissingFileRange { provenance: SmolStr::new("macro_body") })
        );
    }

    #[test]
    fn macro_body_primary_range_is_not_editable_safe() {
        let body = MacroBody {
            definition: MacroDefId(7),
            file_id: Some(FileId(1)),
            range: Some(range(18, 28)),
            token_index: Some(1),
        };
        let provenance = SourceProvenance::MacroBody(body);

        assert_eq!(
            provenance.primary_file_range(),
            Ok(FileRange { file_id: FileId(1), range: range(18, 28) })
        );
        assert_eq!(
            provenance.editable_file_range(),
            Err(ProvenanceUnavailable::NotEditable {
                reason: SmolStr::new("macro body provenance is not editable-safe"),
            })
        );
    }

    #[test]
    fn macro_argument_is_editable_and_relates_to_callsite() {
        let call = MacroCall { use_id: MacroUseId(0), file_id: FileId(1), range: range(30, 44) };
        let argument = MacroArgument {
            call: call.clone(),
            index: 0,
            name: Some(MacroName::new("name")),
            file_id: FileId(1),
            range: range(40, 43),
        };
        let provenance = SourceProvenance::MacroArgument(argument);

        assert_eq!(
            provenance.editable_file_range(),
            Ok(FileRange { file_id: FileId(1), range: range(40, 43) })
        );
        assert_eq!(provenance.related_locations(), vec![SourceProvenance::MacroCall(call)]);
    }

    #[test]
    fn virtual_provenance_has_no_primary_or_editable_range() {
        let virtual_source = VirtualSource::Generated { reason: SmolStr::new("predefined") };
        let provenance = SourceProvenance::Virtual(virtual_source.clone());

        assert_eq!(
            provenance.primary_file_range(),
            Err(ProvenanceUnavailable::Virtual { source: virtual_source.clone() })
        );
        assert_eq!(
            provenance.editable_file_range(),
            Err(ProvenanceUnavailable::Virtual { source: virtual_source })
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
            including_frame: PreprocFrameId(0),
            included_frame: Some(PreprocFrameId(1)),
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
            including_frame: PreprocFrameId(1),
            included_frame: Some(PreprocFrameId(2)),
            parent: Some(IncludeEventId(0)),
            stack: vec![IncludeEventId(0)],
        };
        let trace = PreprocTrace {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            source_instances: vec![
                SourceInstance {
                    id: SourceInstanceId(0),
                    file_id: FileId(0),
                    frame: PreprocFrameId(0),
                },
                SourceInstance {
                    id: SourceInstanceId(1),
                    file_id: FileId(1),
                    frame: PreprocFrameId(1),
                },
                SourceInstance {
                    id: SourceInstanceId(2),
                    file_id: FileId(2),
                    frame: PreprocFrameId(2),
                },
            ],
            frames: vec![
                PreprocFrame {
                    id: PreprocFrameId(0),
                    source_instance: SourceInstanceId(0),
                    file_id: FileId(0),
                    entered_by: None,
                    parent: None,
                    include_stack: Vec::new(),
                },
                PreprocFrame {
                    id: PreprocFrameId(1),
                    source_instance: SourceInstanceId(1),
                    file_id: FileId(1),
                    entered_by: Some(IncludeEventId(0)),
                    parent: Some(PreprocFrameId(0)),
                    include_stack: vec![IncludeEventId(0)],
                },
                PreprocFrame {
                    id: PreprocFrameId(2),
                    source_instance: SourceInstanceId(2),
                    file_id: FileId(2),
                    entered_by: Some(IncludeEventId(1)),
                    parent: Some(PreprocFrameId(1)),
                    include_stack: vec![IncludeEventId(0), IncludeEventId(1)],
                },
            ],
            files: vec![FilePreprocTrace {
                source_instance: SourceInstanceId(2),
                frame: PreprocFrameId(2),
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
        assert_eq!(trace.files[0].source_instance, SourceInstanceId(2));
        assert_eq!(trace.include_events[1].including_frame, PreprocFrameId(1));
        assert_eq!(trace.include_events[1].included_frame, Some(PreprocFrameId(2)));
        assert_eq!(trace.include_events[1].parent, Some(IncludeEventId(0)));
    }

    #[test]
    fn same_file_can_have_multiple_source_instances() {
        let trace = PreprocTrace {
            profile: MacroProfileId(1),
            roots: vec![FileId(0), FileId(2)],
            source_instances: vec![
                SourceInstance {
                    id: SourceInstanceId(0),
                    file_id: FileId(0),
                    frame: PreprocFrameId(0),
                },
                SourceInstance {
                    id: SourceInstanceId(1),
                    file_id: FileId(1),
                    frame: PreprocFrameId(1),
                },
                SourceInstance {
                    id: SourceInstanceId(2),
                    file_id: FileId(2),
                    frame: PreprocFrameId(2),
                },
                SourceInstance {
                    id: SourceInstanceId(3),
                    file_id: FileId(1),
                    frame: PreprocFrameId(3),
                },
            ],
            frames: vec![
                PreprocFrame {
                    id: PreprocFrameId(0),
                    source_instance: SourceInstanceId(0),
                    file_id: FileId(0),
                    entered_by: None,
                    parent: None,
                    include_stack: Vec::new(),
                },
                PreprocFrame {
                    id: PreprocFrameId(1),
                    source_instance: SourceInstanceId(1),
                    file_id: FileId(1),
                    entered_by: Some(IncludeEventId(0)),
                    parent: Some(PreprocFrameId(0)),
                    include_stack: vec![IncludeEventId(0)],
                },
                PreprocFrame {
                    id: PreprocFrameId(2),
                    source_instance: SourceInstanceId(2),
                    file_id: FileId(2),
                    entered_by: None,
                    parent: None,
                    include_stack: Vec::new(),
                },
                PreprocFrame {
                    id: PreprocFrameId(3),
                    source_instance: SourceInstanceId(3),
                    file_id: FileId(1),
                    entered_by: Some(IncludeEventId(1)),
                    parent: Some(PreprocFrameId(2)),
                    include_stack: vec![IncludeEventId(1)],
                },
            ],
            files: vec![
                FilePreprocTrace {
                    source_instance: SourceInstanceId(1),
                    frame: PreprocFrameId(1),
                    file_id: FileId(1),
                    include_stack: vec![IncludeEventId(0)],
                    include_events: Vec::new(),
                    conditional_events: Vec::new(),
                    expansion_events: Vec::new(),
                    expanded_tokens: Vec::new(),
                },
                FilePreprocTrace {
                    source_instance: SourceInstanceId(3),
                    frame: PreprocFrameId(3),
                    file_id: FileId(1),
                    include_stack: vec![IncludeEventId(1)],
                    include_events: Vec::new(),
                    conditional_events: Vec::new(),
                    expansion_events: Vec::new(),
                    expanded_tokens: Vec::new(),
                },
            ],
            include_events: Vec::new(),
            conditional_events: Vec::new(),
            expansion_events: Vec::new(),
            expanded_tokens: Vec::new(),
        };

        let instances_for_header =
            trace.source_instances.iter().filter(|instance| instance.file_id == FileId(1)).count();

        assert_eq!(instances_for_header, 2);
        assert_ne!(trace.files[0].source_instance, trace.files[1].source_instance);
        assert_ne!(trace.files[0].include_stack, trace.files[1].include_stack);
    }

    #[test]
    fn unknown_expanded_token_is_not_an_empty_trace() {
        let trace = PreprocTrace {
            profile: MacroProfileId(1),
            roots: vec![FileId(0)],
            source_instances: Vec::new(),
            frames: Vec::new(),
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
