use crate::{SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxKind, TokenKind, ffi};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTrace {
    pub root_buffer_id: u32,
    pub source_buffers: Vec<SourceBufferId>,
    pub events: Vec<PreprocessorTraceEvent>,
    pub include_edges: Vec<PreprocessorTraceIncludeEdge>,
    pub emitted_tokens: Vec<PreprocessorTraceEmittedToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocessorTraceEventId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocessorTraceMacroCallId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocessorTraceMacroDefinitionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PreprocessorTraceMacroExpansionId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceIncludeEdge {
    pub include_event_id: PreprocessorTraceEventId,
    pub included_buffer_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceEvent {
    pub event_id: PreprocessorTraceEventId,
    pub kind: SyntaxKind,
    pub range: Option<SourceBufferRange>,
    pub macro_definition_id: Option<PreprocessorTraceMacroDefinitionId>,
    pub macro_call_id: Option<PreprocessorTraceMacroCallId>,
    pub macro_expansion_id: Option<PreprocessorTraceMacroExpansionId>,
    pub parent_macro_expansion_id: Option<PreprocessorTraceMacroExpansionId>,
    pub directive: Option<PreprocessorTraceToken>,
    pub name: Option<PreprocessorTraceToken>,
    pub include_file_name: Option<PreprocessorTraceToken>,
    pub params: Vec<PreprocessorTraceMacroParam>,
    pub arguments: Vec<PreprocessorTraceActualArgument>,
    pub body_tokens: Vec<PreprocessorTraceToken>,
    pub expr_tokens: Vec<PreprocessorTraceToken>,
    pub disabled_ranges: Vec<SourceBufferRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceEmittedToken {
    pub raw_text: String,
    pub value_text: String,
    pub display_text: String,
    pub token_kind: TokenKind,
    pub provenance: PreprocessorTraceTokenProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocessorTraceTokenProvenance {
    Source {
        token_range: SourceBufferRange,
    },
    MacroBody {
        macro_name: String,
        identity: PreprocessorTraceMacroBodyIdentity,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
    },
    MacroArgument {
        macro_name: String,
        identity: PreprocessorTraceMacroArgumentIdentity,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
        argument_token_range: SourceBufferRange,
    },
    Builtin {
        name: String,
        identity: PreprocessorTraceMacroBuiltinIdentity,
    },
    TokenPaste {
        identity: PreprocessorTraceMacroOperationIdentity,
    },
    Stringification {
        identity: PreprocessorTraceMacroOperationIdentity,
    },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceMacroBodyIdentity {
    pub call_id: PreprocessorTraceMacroCallId,
    pub definition_id: PreprocessorTraceMacroDefinitionId,
    pub expansion_id: PreprocessorTraceMacroExpansionId,
    pub parent_expansion_id: Option<PreprocessorTraceMacroExpansionId>,
    pub body_token_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceMacroArgumentIdentity {
    pub call_id: PreprocessorTraceMacroCallId,
    pub definition_id: PreprocessorTraceMacroDefinitionId,
    pub expansion_id: PreprocessorTraceMacroExpansionId,
    pub parent_expansion_id: Option<PreprocessorTraceMacroExpansionId>,
    pub body_token_index: u32,
    pub argument_index: u32,
    pub argument_token_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceMacroBuiltinIdentity {
    pub call_id: PreprocessorTraceMacroCallId,
    pub expansion_id: PreprocessorTraceMacroExpansionId,
    pub parent_expansion_id: Option<PreprocessorTraceMacroExpansionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceMacroOperationIdentity {
    pub call_id: PreprocessorTraceMacroCallId,
    pub definition_id: PreprocessorTraceMacroDefinitionId,
    pub expansion_id: PreprocessorTraceMacroExpansionId,
    pub parent_expansion_id: Option<PreprocessorTraceMacroExpansionId>,
    pub body_token_index: u32,
    pub argument_index: Option<u32>,
    pub argument_token_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceToken {
    pub raw_text: String,
    pub value_text: String,
    pub token_kind: TokenKind,
    pub range: Option<SourceBufferRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceMacroParam {
    pub name: Option<PreprocessorTraceToken>,
    pub default_tokens: Option<Vec<PreprocessorTraceToken>>,
    pub range: Option<SourceBufferRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorTraceActualArgument {
    pub tokens: Vec<PreprocessorTraceToken>,
    pub range: Option<SourceBufferRange>,
}

impl PreprocessorTrace {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawPreprocessorTrace) -> Option<Self> {
        raw.has_root_buffer_id.then(|| Self {
            root_buffer_id: raw.root_buffer_id,
            source_buffers: raw
                .source_buffers
                .into_iter()
                .map(|buffer| SourceBufferId {
                    path: buffer.path,
                    text: buffer.has_text.then_some(buffer.text),
                    buffer_id: buffer.buffer_id,
                    origin: SourceBufferOrigin::from_raw(buffer.origin),
                })
                .collect(),
            events: raw.events.into_iter().map(PreprocessorTraceEvent::from_raw).collect(),
            include_edges: raw
                .include_edges
                .into_iter()
                .map(|edge| PreprocessorTraceIncludeEdge {
                    include_event_id: PreprocessorTraceEventId(edge.include_event_id),
                    included_buffer_id: edge.included_buffer_id,
                })
                .collect(),
            emitted_tokens: raw
                .emitted_tokens
                .into_iter()
                .map(PreprocessorTraceEmittedToken::from_raw)
                .collect(),
        })
    }
}

impl PreprocessorTraceEvent {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceEvent) -> Self {
        Self {
            event_id: PreprocessorTraceEventId(raw.event_id),
            kind: SyntaxKind::from_id(raw.kind),
            range: SourceBufferRange::from_raw(raw.range),
            macro_definition_id: raw
                .has_macro_definition_id
                .then_some(PreprocessorTraceMacroDefinitionId(raw.macro_definition_id)),
            macro_call_id: raw
                .has_macro_call_id
                .then_some(PreprocessorTraceMacroCallId(raw.macro_call_id)),
            macro_expansion_id: raw
                .has_macro_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.macro_expansion_id)),
            parent_macro_expansion_id: raw
                .has_parent_macro_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.parent_macro_expansion_id)),
            directive: PreprocessorTraceToken::from_raw(raw.directive),
            name: PreprocessorTraceToken::from_raw(raw.name),
            include_file_name: PreprocessorTraceToken::from_raw(raw.include_file_name),
            params: raw.params.into_iter().map(PreprocessorTraceMacroParam::from_raw).collect(),
            arguments: raw
                .arguments
                .into_iter()
                .map(PreprocessorTraceActualArgument::from_raw)
                .collect(),
            body_tokens: raw
                .body_tokens
                .into_iter()
                .filter_map(PreprocessorTraceToken::from_raw)
                .collect(),
            expr_tokens: raw
                .expr_tokens
                .into_iter()
                .filter_map(PreprocessorTraceToken::from_raw)
                .collect(),
            disabled_ranges: raw
                .disabled_ranges
                .into_iter()
                .filter_map(SourceBufferRange::from_raw)
                .collect(),
        }
    }
}

impl PreprocessorTraceEmittedToken {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawPreprocessorTraceEmittedToken) -> Self {
        let ffi::RawPreprocessorTraceEmittedToken {
            raw_text,
            value_text,
            display_text,
            token_kind,
            provenance_kind,
            macro_name,
            macro_call_id,
            has_macro_call_id,
            macro_definition_id,
            has_macro_definition_id,
            macro_expansion_id,
            has_macro_expansion_id,
            parent_macro_expansion_id,
            has_parent_macro_expansion_id,
            body_token_index,
            has_body_token_index,
            argument_index,
            has_argument_index,
            argument_token_index,
            has_argument_token_index,
            token_range,
            call_range,
            body_token_range,
            argument_token_range,
        } = raw;
        Self {
            raw_text,
            value_text,
            display_text,
            token_kind: TokenKind::from_id(token_kind),
            provenance: PreprocessorTraceTokenProvenance::from_raw(
                RawPreprocessorTraceTokenProvenance {
                    kind: provenance_kind,
                    macro_name,
                    identity: RawPreprocessorTraceMacroIdentity {
                        call_id: macro_call_id,
                        has_call_id: has_macro_call_id,
                        definition_id: macro_definition_id,
                        has_definition_id: has_macro_definition_id,
                        expansion_id: macro_expansion_id,
                        has_expansion_id: has_macro_expansion_id,
                        parent_expansion_id: parent_macro_expansion_id,
                        has_parent_expansion_id: has_parent_macro_expansion_id,
                        body_token_index,
                        has_body_token_index,
                        argument_index,
                        has_argument_index,
                        argument_token_index,
                        has_argument_token_index,
                    },
                    token_range,
                    call_range,
                    body_token_range,
                    argument_token_range,
                },
            ),
        }
    }
}

struct RawPreprocessorTraceTokenProvenance {
    kind: u8,
    macro_name: String,
    identity: RawPreprocessorTraceMacroIdentity,
    token_range: ffi::RawSourceBufferRange,
    call_range: ffi::RawSourceBufferRange,
    body_token_range: ffi::RawSourceBufferRange,
    argument_token_range: ffi::RawSourceBufferRange,
}

struct RawPreprocessorTraceMacroIdentity {
    call_id: u32,
    has_call_id: bool,
    definition_id: u32,
    has_definition_id: bool,
    expansion_id: u32,
    has_expansion_id: bool,
    parent_expansion_id: u32,
    has_parent_expansion_id: bool,
    body_token_index: u32,
    has_body_token_index: bool,
    argument_index: u32,
    has_argument_index: bool,
    argument_token_index: u32,
    has_argument_token_index: bool,
}

impl PreprocessorTraceTokenProvenance {
    const BUILTIN: u8 = 4;
    const MACRO_ARGUMENT: u8 = 3;
    const MACRO_BODY: u8 = 2;
    const SOURCE: u8 = 1;
    const STRINGIFICATION: u8 = 6;
    const TOKEN_PASTE: u8 = 5;
    const UNAVAILABLE: u8 = 0;

    #[inline]
    fn from_raw(raw: RawPreprocessorTraceTokenProvenance) -> Self {
        match raw.kind {
            Self::SOURCE => SourceBufferRange::from_raw(raw.token_range)
                .map(|token_range| Self::Source { token_range })
                .unwrap_or(Self::Unavailable),
            Self::MACRO_BODY => {
                let Some(call_range) = SourceBufferRange::from_raw(raw.call_range) else {
                    return Self::Unavailable;
                };
                let Some(body_token_range) = SourceBufferRange::from_raw(raw.body_token_range)
                else {
                    return Self::Unavailable;
                };
                let Some(identity) = PreprocessorTraceMacroBodyIdentity::from_raw(&raw.identity)
                else {
                    return Self::Unavailable;
                };
                Self::MacroBody {
                    macro_name: raw.macro_name,
                    identity,
                    call_range,
                    body_token_range,
                }
            }
            Self::MACRO_ARGUMENT => {
                let Some(call_range) = SourceBufferRange::from_raw(raw.call_range) else {
                    return Self::Unavailable;
                };
                let Some(body_token_range) = SourceBufferRange::from_raw(raw.body_token_range)
                else {
                    return Self::Unavailable;
                };
                let Some(argument_token_range) =
                    SourceBufferRange::from_raw(raw.argument_token_range)
                else {
                    return Self::Unavailable;
                };
                let Some(identity) =
                    PreprocessorTraceMacroArgumentIdentity::from_raw(&raw.identity)
                else {
                    return Self::Unavailable;
                };
                Self::MacroArgument {
                    macro_name: raw.macro_name,
                    identity,
                    call_range,
                    body_token_range,
                    argument_token_range,
                }
            }
            Self::BUILTIN if !raw.macro_name.is_empty() => {
                let Some(identity) = PreprocessorTraceMacroBuiltinIdentity::from_raw(&raw.identity)
                else {
                    return Self::Unavailable;
                };
                Self::Builtin { name: raw.macro_name, identity }
            }
            Self::TOKEN_PASTE => {
                let Some(identity) =
                    PreprocessorTraceMacroOperationIdentity::from_raw(&raw.identity)
                else {
                    return Self::Unavailable;
                };
                Self::TokenPaste { identity }
            }
            Self::STRINGIFICATION => {
                let Some(identity) =
                    PreprocessorTraceMacroOperationIdentity::from_raw(&raw.identity)
                else {
                    return Self::Unavailable;
                };
                Self::Stringification { identity }
            }
            Self::UNAVAILABLE => Self::Unavailable,
            _ => Self::Unavailable,
        }
    }
}

impl PreprocessorTraceMacroBodyIdentity {
    #[inline]
    fn from_raw(raw: &RawPreprocessorTraceMacroIdentity) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(PreprocessorTraceMacroCallId(raw.call_id))?,
            definition_id: raw
                .has_definition_id
                .then_some(PreprocessorTraceMacroDefinitionId(raw.definition_id))?,
            expansion_id: raw
                .has_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.parent_expansion_id)),
            body_token_index: raw.has_body_token_index.then_some(raw.body_token_index)?,
        })
    }
}

impl PreprocessorTraceMacroArgumentIdentity {
    #[inline]
    fn from_raw(raw: &RawPreprocessorTraceMacroIdentity) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(PreprocessorTraceMacroCallId(raw.call_id))?,
            definition_id: raw
                .has_definition_id
                .then_some(PreprocessorTraceMacroDefinitionId(raw.definition_id))?,
            expansion_id: raw
                .has_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.parent_expansion_id)),
            body_token_index: raw.has_body_token_index.then_some(raw.body_token_index)?,
            argument_index: raw.has_argument_index.then_some(raw.argument_index)?,
            argument_token_index: raw
                .has_argument_token_index
                .then_some(raw.argument_token_index)?,
        })
    }
}

impl PreprocessorTraceMacroBuiltinIdentity {
    #[inline]
    fn from_raw(raw: &RawPreprocessorTraceMacroIdentity) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(PreprocessorTraceMacroCallId(raw.call_id))?,
            expansion_id: raw
                .has_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.parent_expansion_id)),
        })
    }
}

impl PreprocessorTraceMacroOperationIdentity {
    #[inline]
    fn from_raw(raw: &RawPreprocessorTraceMacroIdentity) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(PreprocessorTraceMacroCallId(raw.call_id))?,
            definition_id: raw
                .has_definition_id
                .then_some(PreprocessorTraceMacroDefinitionId(raw.definition_id))?,
            expansion_id: raw
                .has_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(PreprocessorTraceMacroExpansionId(raw.parent_expansion_id)),
            body_token_index: raw.has_body_token_index.then_some(raw.body_token_index)?,
            argument_index: raw.has_argument_index.then_some(raw.argument_index),
            argument_token_index: raw.has_argument_token_index.then_some(raw.argument_token_index),
        })
    }
}

impl PreprocessorTraceToken {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceToken) -> Option<Self> {
        raw.has_token.then(|| Self {
            raw_text: raw.raw_text,
            value_text: raw.value_text,
            token_kind: TokenKind::from_id(raw.token_kind),
            range: SourceBufferRange::from_raw(raw.range),
        })
    }
}

impl PreprocessorTraceMacroParam {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceMacroParam) -> Self {
        Self {
            name: PreprocessorTraceToken::from_raw(raw.name),
            default_tokens: raw.has_default.then(|| {
                raw.default_tokens
                    .into_iter()
                    .filter_map(PreprocessorTraceToken::from_raw)
                    .collect()
            }),
            range: SourceBufferRange::from_raw(raw.range),
        }
    }
}

impl PreprocessorTraceActualArgument {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceActualArgument) -> Self {
        Self {
            tokens: raw.tokens.into_iter().filter_map(PreprocessorTraceToken::from_raw).collect(),
            range: SourceBufferRange::from_raw(raw.range),
        }
    }
}
