use crate::{SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxKind, TokenKind, ffi};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub root_buffer_id: u32,
    pub source_buffers: Vec<SourceBufferId>,
    pub events: Vec<Event>,
    pub include_edges: Vec<IncludeEdge>,
    pub emitted_tokens: Vec<EmittedToken>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacroCallId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacroDefinitionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MacroExpansionId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEdge {
    pub include_event_id: EventId,
    pub included_buffer_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub event_id: EventId,
    pub kind: SyntaxKind,
    pub range: Option<SourceBufferRange>,
    pub macro_definition_id: Option<MacroDefinitionId>,
    pub macro_call_id: Option<MacroCallId>,
    pub macro_expansion_id: Option<MacroExpansionId>,
    pub parent_macro_expansion_id: Option<MacroExpansionId>,
    pub directive: Option<Token>,
    pub name: Option<Token>,
    pub include_file_name: Option<Token>,
    pub params: Vec<MacroParam>,
    pub arguments: Vec<ActualArgument>,
    pub body_tokens: Vec<Token>,
    pub expr_tokens: Vec<Token>,
    pub disabled_ranges: Vec<SourceBufferRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedToken {
    pub raw_text: String,
    pub value_text: String,
    pub display_text: String,
    pub token_kind: TokenKind,
    pub origin: TokenOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenOrigin {
    Source {
        token_range: SourceBufferRange,
    },
    MacroBody {
        macro_name: String,
        origin: MacroBodyOrigin,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
    },
    MacroArgument {
        macro_name: String,
        origin: MacroArgumentOrigin,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
        argument_token_range: SourceBufferRange,
    },
    Builtin {
        name: String,
        origin: MacroBuiltinOrigin,
    },
    TokenPaste {
        origin: MacroOperationOrigin,
    },
    Stringification {
        origin: MacroOperationOrigin,
    },
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroBodyOrigin {
    pub call_id: MacroCallId,
    pub definition_id: MacroDefinitionId,
    pub expansion_id: MacroExpansionId,
    pub parent_expansion_id: Option<MacroExpansionId>,
    pub body_token_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroArgumentOrigin {
    pub call_id: MacroCallId,
    pub definition_id: MacroDefinitionId,
    pub expansion_id: MacroExpansionId,
    pub parent_expansion_id: Option<MacroExpansionId>,
    pub body_token_index: u32,
    pub argument_index: u32,
    pub argument_token_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroBuiltinOrigin {
    pub call_id: MacroCallId,
    pub expansion_id: MacroExpansionId,
    pub parent_expansion_id: Option<MacroExpansionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroOperationOrigin {
    pub call_id: MacroCallId,
    pub definition_id: MacroDefinitionId,
    pub expansion_id: MacroExpansionId,
    pub parent_expansion_id: Option<MacroExpansionId>,
    pub body_token_index: u32,
    pub argument_index: Option<u32>,
    pub argument_token_index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub raw_text: String,
    pub value_text: String,
    pub token_kind: TokenKind,
    pub range: Option<SourceBufferRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParam {
    pub name: Option<Token>,
    pub default_tokens: Option<Vec<Token>>,
    pub range: Option<SourceBufferRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActualArgument {
    pub tokens: Vec<Token>,
    pub range: Option<SourceBufferRange>,
}

impl Trace {
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
            events: raw.events.into_iter().map(Event::from_raw).collect(),
            include_edges: raw
                .include_edges
                .into_iter()
                .map(|edge| IncludeEdge {
                    include_event_id: EventId(edge.include_event_id),
                    included_buffer_id: edge.included_buffer_id,
                })
                .collect(),
            emitted_tokens: raw.emitted_tokens.into_iter().map(EmittedToken::from_raw).collect(),
        })
    }
}

impl Event {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceEvent) -> Self {
        Self {
            event_id: EventId(raw.event_id),
            kind: SyntaxKind::from_id(raw.kind),
            range: SourceBufferRange::from_raw(raw.range),
            macro_definition_id: raw
                .has_macro_definition_id
                .then_some(MacroDefinitionId(raw.macro_definition_id)),
            macro_call_id: raw.has_macro_call_id.then_some(MacroCallId(raw.macro_call_id)),
            macro_expansion_id: raw
                .has_macro_expansion_id
                .then_some(MacroExpansionId(raw.macro_expansion_id)),
            parent_macro_expansion_id: raw
                .has_parent_macro_expansion_id
                .then_some(MacroExpansionId(raw.parent_macro_expansion_id)),
            directive: Token::from_raw(raw.directive),
            name: Token::from_raw(raw.name),
            include_file_name: Token::from_raw(raw.include_file_name),
            params: raw.params.into_iter().map(MacroParam::from_raw).collect(),
            arguments: raw.arguments.into_iter().map(ActualArgument::from_raw).collect(),
            body_tokens: raw.body_tokens.into_iter().filter_map(Token::from_raw).collect(),
            expr_tokens: raw.expr_tokens.into_iter().filter_map(Token::from_raw).collect(),
            disabled_ranges: raw
                .disabled_ranges
                .into_iter()
                .filter_map(SourceBufferRange::from_raw)
                .collect(),
        }
    }
}

impl EmittedToken {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawPreprocessorTraceEmittedToken) -> Self {
        let ffi::RawPreprocessorTraceEmittedToken {
            raw_text,
            value_text,
            display_text,
            token_kind,
            origin_kind,
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
            origin: TokenOrigin::from_raw(RawTokenOrigin {
                kind: origin_kind,
                macro_name,
                origin: RawMacroOrigin {
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
            }),
        }
    }
}

struct RawTokenOrigin {
    kind: u8,
    macro_name: String,
    origin: RawMacroOrigin,
    token_range: ffi::RawSourceBufferRange,
    call_range: ffi::RawSourceBufferRange,
    body_token_range: ffi::RawSourceBufferRange,
    argument_token_range: ffi::RawSourceBufferRange,
}

struct RawMacroOrigin {
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

impl TokenOrigin {
    const BUILTIN: u8 = 4;
    const MACRO_ARGUMENT: u8 = 3;
    const MACRO_BODY: u8 = 2;
    const SOURCE: u8 = 1;
    const STRINGIFICATION: u8 = 6;
    const TOKEN_PASTE: u8 = 5;
    const UNAVAILABLE: u8 = 0;

    #[inline]
    fn from_raw(raw: RawTokenOrigin) -> Self {
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
                let Some(origin) = MacroBodyOrigin::from_raw(&raw.origin) else {
                    return Self::Unavailable;
                };
                Self::MacroBody { macro_name: raw.macro_name, origin, call_range, body_token_range }
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
                let Some(origin) = MacroArgumentOrigin::from_raw(&raw.origin) else {
                    return Self::Unavailable;
                };
                Self::MacroArgument {
                    macro_name: raw.macro_name,
                    origin,
                    call_range,
                    body_token_range,
                    argument_token_range,
                }
            }
            Self::BUILTIN if !raw.macro_name.is_empty() => {
                let Some(origin) = MacroBuiltinOrigin::from_raw(&raw.origin) else {
                    return Self::Unavailable;
                };
                Self::Builtin { name: raw.macro_name, origin }
            }
            Self::TOKEN_PASTE => {
                let Some(origin) = MacroOperationOrigin::from_raw(&raw.origin) else {
                    return Self::Unavailable;
                };
                Self::TokenPaste { origin }
            }
            Self::STRINGIFICATION => {
                let Some(origin) = MacroOperationOrigin::from_raw(&raw.origin) else {
                    return Self::Unavailable;
                };
                Self::Stringification { origin }
            }
            Self::UNAVAILABLE => Self::Unavailable,
            _ => Self::Unavailable,
        }
    }
}

impl MacroBodyOrigin {
    #[inline]
    fn from_raw(raw: &RawMacroOrigin) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(MacroCallId(raw.call_id))?,
            definition_id: raw.has_definition_id.then_some(MacroDefinitionId(raw.definition_id))?,
            expansion_id: raw.has_expansion_id.then_some(MacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(MacroExpansionId(raw.parent_expansion_id)),
            body_token_index: raw.has_body_token_index.then_some(raw.body_token_index)?,
        })
    }
}

impl MacroArgumentOrigin {
    #[inline]
    fn from_raw(raw: &RawMacroOrigin) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(MacroCallId(raw.call_id))?,
            definition_id: raw.has_definition_id.then_some(MacroDefinitionId(raw.definition_id))?,
            expansion_id: raw.has_expansion_id.then_some(MacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(MacroExpansionId(raw.parent_expansion_id)),
            body_token_index: raw.has_body_token_index.then_some(raw.body_token_index)?,
            argument_index: raw.has_argument_index.then_some(raw.argument_index)?,
            argument_token_index: raw
                .has_argument_token_index
                .then_some(raw.argument_token_index)?,
        })
    }
}

impl MacroBuiltinOrigin {
    #[inline]
    fn from_raw(raw: &RawMacroOrigin) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(MacroCallId(raw.call_id))?,
            expansion_id: raw.has_expansion_id.then_some(MacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(MacroExpansionId(raw.parent_expansion_id)),
        })
    }
}

impl MacroOperationOrigin {
    #[inline]
    fn from_raw(raw: &RawMacroOrigin) -> Option<Self> {
        Some(Self {
            call_id: raw.has_call_id.then_some(MacroCallId(raw.call_id))?,
            definition_id: raw.has_definition_id.then_some(MacroDefinitionId(raw.definition_id))?,
            expansion_id: raw.has_expansion_id.then_some(MacroExpansionId(raw.expansion_id))?,
            parent_expansion_id: raw
                .has_parent_expansion_id
                .then_some(MacroExpansionId(raw.parent_expansion_id)),
            body_token_index: raw.has_body_token_index.then_some(raw.body_token_index)?,
            argument_index: raw.has_argument_index.then_some(raw.argument_index),
            argument_token_index: raw.has_argument_token_index.then_some(raw.argument_token_index),
        })
    }
}

impl Token {
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

impl MacroParam {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceMacroParam) -> Self {
        Self {
            name: Token::from_raw(raw.name),
            default_tokens: raw
                .has_default
                .then(|| raw.default_tokens.into_iter().filter_map(Token::from_raw).collect()),
            range: SourceBufferRange::from_raw(raw.range),
        }
    }
}

impl ActualArgument {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorTraceActualArgument) -> Self {
        Self {
            tokens: raw.tokens.into_iter().filter_map(Token::from_raw).collect(),
            range: SourceBufferRange::from_raw(raw.range),
        }
    }
}
