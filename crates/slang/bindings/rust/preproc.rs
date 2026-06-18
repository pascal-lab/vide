use crate::{SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxKind, TokenKind, ffi};

mod origin;

pub use origin::TokenOrigin;

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
