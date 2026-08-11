use super::{SourceBufferId, SourceBufferRange};
use crate::{
    syntax::{SyntaxKind, ffi},
    token::TokenKind,
};

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
    pub macro_origin: MacroOrigin,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroOrigin {
    Unknown,
    Source,
    Predefine,
    BuiltIn,
    Intrinsic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedToken {
    pub emitted_token_index: Option<u32>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceTokenOrigin {
    pub kind: u8,
    pub macro_name: String,
    pub call_id: Option<MacroCallId>,
    pub definition_id: Option<MacroDefinitionId>,
    pub expansion_id: Option<MacroExpansionId>,
    pub parent_expansion_id: Option<MacroExpansionId>,
    pub body_token_index: Option<u32>,
    pub argument_index: Option<u32>,
    pub argument_token_index: Option<u32>,
    pub token_range: Option<SourceBufferRange>,
    pub call_range: Option<SourceBufferRange>,
    pub body_token_range: Option<SourceBufferRange>,
    pub argument_token_range: Option<SourceBufferRange>,
}

impl TraceTokenOrigin {
    pub const BUILTIN: u8 = 4;
    pub const MACRO_ARGUMENT: u8 = 3;
    pub const MACRO_BODY: u8 = 2;
    pub const PREDEFINE: u8 = 7;
    pub const SOURCE: u8 = 1;
    pub const STRINGIFICATION: u8 = 6;
    pub const TOKEN_PASTE: u8 = 5;
    pub const UNAVAILABLE: u8 = 0;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenOrigin {
    Source {
        token_range: SourceBufferRange,
    },
    MacroBody {
        macro_name: String,
        call_id: MacroCallId,
        definition_id: MacroDefinitionId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
    },
    MacroArgument {
        macro_name: String,
        call_id: MacroCallId,
        definition_id: MacroDefinitionId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        argument_index: u32,
        argument_token_index: u32,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
        argument_token_range: SourceBufferRange,
    },
    Predefine {
        macro_name: String,
        call_id: MacroCallId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        argument_index: Option<u32>,
        argument_token_index: Option<u32>,
        call_range: SourceBufferRange,
        body_token_range: SourceBufferRange,
        argument_token_range: Option<SourceBufferRange>,
    },
    Builtin {
        name: String,
        call_id: MacroCallId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
    },
    TokenPaste {
        call_id: MacroCallId,
        definition_id: Option<MacroDefinitionId>,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        argument_index: Option<u32>,
        argument_token_index: Option<u32>,
    },
    Stringify {
        call_id: MacroCallId,
        definition_id: Option<MacroDefinitionId>,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        argument_index: Option<u32>,
        argument_token_index: Option<u32>,
    },
    Unavailable,
}

impl Trace {
    pub(crate) fn from_raw(raw: ffi::RawTrace) -> Self {
        Self {
            root_buffer_id: raw.root_buffer_id,
            source_buffers: raw
                .source_buffers
                .into_iter()
                .map(|buffer| {
                    let origin = match buffer.origin {
                        0 => super::SourceBufferOrigin::Source,
                        1 => super::SourceBufferOrigin::Predefine,
                        origin => {
                            panic!("Slang returned an unknown trace source buffer origin: {origin}")
                        }
                    };
                    SourceBufferId {
                        path: buffer.path,
                        text: Some(buffer.text),
                        buffer_id: buffer.buffer_id,
                        origin,
                    }
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
        }
    }
}

impl Event {
    fn from_raw(raw: ffi::RawTraceEvent) -> Self {
        Self {
            event_id: EventId(raw.event_id),
            kind: SyntaxKind::from_raw(raw.kind),
            range: range_from_raw(raw.range),
            macro_origin: MacroOrigin::from_raw(raw.macro_origin),
            macro_definition_id: optional_id(
                raw.has_macro_definition_id,
                raw.macro_definition_id,
                MacroDefinitionId,
            ),
            macro_call_id: optional_id(raw.has_macro_call_id, raw.macro_call_id, MacroCallId),
            macro_expansion_id: optional_id(
                raw.has_macro_expansion_id,
                raw.macro_expansion_id,
                MacroExpansionId,
            ),
            parent_macro_expansion_id: optional_id(
                raw.has_parent_macro_expansion_id,
                raw.parent_macro_expansion_id,
                MacroExpansionId,
            ),
            directive: token_from_raw(raw.directive),
            name: token_from_raw(raw.name),
            include_file_name: token_from_raw(raw.include_file_name),
            params: raw.params.into_iter().map(MacroParam::from_raw).collect(),
            arguments: raw.arguments.into_iter().map(ActualArgument::from_raw).collect(),
            body_tokens: raw.body_tokens.into_iter().filter_map(token_from_raw).collect(),
            expr_tokens: raw.expr_tokens.into_iter().filter_map(token_from_raw).collect(),
            disabled_ranges: raw.disabled_ranges.into_iter().filter_map(range_from_raw).collect(),
        }
    }
}

impl MacroOrigin {
    fn from_raw(raw: u8) -> Self {
        match raw {
            0 => Self::Unknown,
            1 => Self::Source,
            2 => Self::Predefine,
            3 => Self::BuiltIn,
            4 => Self::Intrinsic,
            raw => panic!("Slang returned an unknown macro origin: {raw}"),
        }
    }
}

impl EmittedToken {
    pub(crate) fn from_raw(raw: ffi::RawTraceEmittedToken) -> Self {
        Self {
            emitted_token_index: raw.has_emitted_token_index.then_some(raw.emitted_token_index),
            raw_text: raw.raw_text,
            value_text: raw.value_text,
            display_text: raw.display_text,
            token_kind: TokenKind::from_raw(raw.token_kind),
            origin: TokenOrigin::from_raw(raw.origin),
        }
    }
}

impl TokenOrigin {
    fn from_raw(raw: ffi::RawTraceTokenOrigin) -> Self {
        let call_id = optional_id(raw.has_macro_call_id, raw.macro_call_id, MacroCallId);
        let definition_id =
            optional_id(raw.has_macro_definition_id, raw.macro_definition_id, MacroDefinitionId);
        let expansion_id =
            optional_id(raw.has_macro_expansion_id, raw.macro_expansion_id, MacroExpansionId);
        let parent_expansion_id = optional_id(
            raw.has_parent_macro_expansion_id,
            raw.parent_macro_expansion_id,
            MacroExpansionId,
        );
        match raw.kind {
            TraceTokenOrigin::SOURCE => {
                TokenOrigin::Source { token_range: range_required(raw.token_range) }
            }
            TraceTokenOrigin::MACRO_BODY => TokenOrigin::MacroBody {
                macro_name: raw.macro_name,
                call_id: call_id.expect("Slang macro body origin has no call id"),
                definition_id: definition_id.expect("Slang macro body origin has no definition id"),
                expansion_id: expansion_id.expect("Slang macro body origin has no expansion id"),
                parent_expansion_id,
                body_token_index: raw
                    .has_body_token_index
                    .then_some(raw.body_token_index)
                    .expect("Slang macro body origin has no body token index"),
                call_range: range_required(raw.call_range),
                body_token_range: range_required(raw.body_token_range),
            },
            TraceTokenOrigin::MACRO_ARGUMENT => TokenOrigin::MacroArgument {
                macro_name: raw.macro_name,
                call_id: call_id.expect("Slang macro argument origin has no call id"),
                definition_id: definition_id
                    .expect("Slang macro argument origin has no definition id"),
                expansion_id: expansion_id
                    .expect("Slang macro argument origin has no expansion id"),
                parent_expansion_id,
                body_token_index: raw
                    .has_body_token_index
                    .then_some(raw.body_token_index)
                    .expect("Slang macro argument origin has no body token index"),
                argument_index: raw
                    .has_argument_index
                    .then_some(raw.argument_index)
                    .expect("Slang macro argument origin has no argument index"),
                argument_token_index: raw
                    .has_argument_token_index
                    .then_some(raw.argument_token_index)
                    .expect("Slang macro argument origin has no argument token index"),
                call_range: range_required(raw.call_range),
                body_token_range: range_required(raw.body_token_range),
                argument_token_range: range_required(raw.argument_token_range),
            },
            TraceTokenOrigin::PREDEFINE => TokenOrigin::Predefine {
                macro_name: raw.macro_name,
                call_id: call_id.expect("Slang predefine origin has no call id"),
                expansion_id: expansion_id.expect("Slang predefine origin has no expansion id"),
                parent_expansion_id,
                body_token_index: raw
                    .has_body_token_index
                    .then_some(raw.body_token_index)
                    .expect("Slang predefine origin has no body token index"),
                argument_index: raw.has_argument_index.then_some(raw.argument_index),
                argument_token_index: raw
                    .has_argument_token_index
                    .then_some(raw.argument_token_index),
                call_range: range_required(raw.call_range),
                body_token_range: range_required(raw.body_token_range),
                argument_token_range: raw
                    .has_argument_token_index
                    .then(|| range_required(raw.argument_token_range)),
            },
            TraceTokenOrigin::BUILTIN => TokenOrigin::Builtin {
                name: raw.macro_name,
                call_id: call_id.expect("Slang builtin origin has no call id"),
                expansion_id: expansion_id.expect("Slang builtin origin has no expansion id"),
                parent_expansion_id,
            },
            TraceTokenOrigin::TOKEN_PASTE => TokenOrigin::TokenPaste {
                call_id: call_id.expect("Slang token paste origin has no call id"),
                definition_id,
                expansion_id: expansion_id.expect("Slang token paste origin has no expansion id"),
                parent_expansion_id,
                body_token_index: raw.body_token_index,
                argument_index: raw.has_argument_index.then_some(raw.argument_index),
                argument_token_index: raw
                    .has_argument_token_index
                    .then_some(raw.argument_token_index),
            },
            TraceTokenOrigin::STRINGIFICATION => TokenOrigin::Stringify {
                call_id: call_id.expect("Slang stringification origin has no call id"),
                definition_id,
                expansion_id: expansion_id
                    .expect("Slang stringification origin has no expansion id"),
                parent_expansion_id,
                body_token_index: raw.body_token_index,
                argument_index: raw.has_argument_index.then_some(raw.argument_index),
                argument_token_index: raw
                    .has_argument_token_index
                    .then_some(raw.argument_token_index),
            },
            TraceTokenOrigin::UNAVAILABLE => TokenOrigin::Unavailable,
            kind => panic!("Slang returned an unknown trace token origin: {kind}"),
        }
    }
}

impl MacroParam {
    fn from_raw(raw: ffi::RawTraceMacroParam) -> Self {
        Self {
            name: token_from_raw(raw.name),
            default_tokens: raw
                .has_default
                .then(|| raw.default_tokens.into_iter().filter_map(token_from_raw).collect()),
            range: range_from_raw(raw.range),
        }
    }
}

impl ActualArgument {
    fn from_raw(raw: ffi::RawTraceActualArgument) -> Self {
        Self {
            tokens: raw.tokens.into_iter().filter_map(token_from_raw).collect(),
            range: range_from_raw(raw.range),
        }
    }
}

fn optional_id<T>(has_id: bool, id: u32, make: fn(u32) -> T) -> Option<T> {
    has_id.then(|| make(id))
}

fn range_from_raw(raw: ffi::RawTraceSourceRange) -> Option<SourceBufferRange> {
    raw.has_range.then_some(SourceBufferRange {
        buffer_id: raw.buffer_id,
        range: raw.range_start..raw.range_end,
    })
}

fn range_required(raw: ffi::RawTraceSourceRange) -> SourceBufferRange {
    range_from_raw(raw).expect("Slang trace origin is missing a source range")
}

fn token_from_raw(raw: ffi::RawTraceToken) -> Option<Token> {
    raw.has_token.then_some(Token {
        raw_text: raw.raw_text,
        value_text: raw.value_text,
        token_kind: TokenKind::from_raw(raw.token_kind),
        range: range_from_raw(raw.range),
    })
}
