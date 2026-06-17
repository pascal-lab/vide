mod raw;

use raw::RawTokenOrigin;

use super::{MacroCallId, MacroDefinitionId, MacroExpansionId};
use crate::SourceBufferRange;

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
    Builtin {
        name: String,
        call_id: MacroCallId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
    },
    TokenPaste {
        call_id: MacroCallId,
        definition_id: MacroDefinitionId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        argument_index: Option<u32>,
        argument_token_index: Option<u32>,
    },
    Stringify {
        call_id: MacroCallId,
        definition_id: MacroDefinitionId,
        expansion_id: MacroExpansionId,
        parent_expansion_id: Option<MacroExpansionId>,
        body_token_index: u32,
        argument_index: Option<u32>,
        argument_token_index: Option<u32>,
    },
    Unavailable,
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
                let Some(call_id) = raw.origin.call_id() else {
                    return Self::Unavailable;
                };
                let Some(definition_id) = raw.origin.definition_id() else {
                    return Self::Unavailable;
                };
                let Some(expansion_id) = raw.origin.expansion_id() else {
                    return Self::Unavailable;
                };
                let Some(body_token_index) = raw.origin.body_token_index() else {
                    return Self::Unavailable;
                };
                Self::MacroBody {
                    macro_name: raw.macro_name,
                    call_id,
                    definition_id,
                    expansion_id,
                    parent_expansion_id: raw.origin.parent_expansion_id(),
                    body_token_index,
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
                let Some(call_id) = raw.origin.call_id() else {
                    return Self::Unavailable;
                };
                let Some(definition_id) = raw.origin.definition_id() else {
                    return Self::Unavailable;
                };
                let Some(expansion_id) = raw.origin.expansion_id() else {
                    return Self::Unavailable;
                };
                let Some(body_token_index) = raw.origin.body_token_index() else {
                    return Self::Unavailable;
                };
                let Some(argument_index) = raw.origin.argument_index() else {
                    return Self::Unavailable;
                };
                let Some(argument_token_index) = raw.origin.argument_token_index() else {
                    return Self::Unavailable;
                };
                Self::MacroArgument {
                    macro_name: raw.macro_name,
                    call_id,
                    definition_id,
                    expansion_id,
                    parent_expansion_id: raw.origin.parent_expansion_id(),
                    body_token_index,
                    argument_index,
                    argument_token_index,
                    call_range,
                    body_token_range,
                    argument_token_range,
                }
            }
            Self::BUILTIN if !raw.macro_name.is_empty() => {
                let Some(call_id) = raw.origin.call_id() else {
                    return Self::Unavailable;
                };
                let Some(expansion_id) = raw.origin.expansion_id() else {
                    return Self::Unavailable;
                };
                Self::Builtin {
                    name: raw.macro_name,
                    call_id,
                    expansion_id,
                    parent_expansion_id: raw.origin.parent_expansion_id(),
                }
            }
            Self::TOKEN_PASTE => {
                let Some(call_id) = raw.origin.call_id() else {
                    return Self::Unavailable;
                };
                let Some(definition_id) = raw.origin.definition_id() else {
                    return Self::Unavailable;
                };
                let Some(expansion_id) = raw.origin.expansion_id() else {
                    return Self::Unavailable;
                };
                let Some(body_token_index) = raw.origin.body_token_index() else {
                    return Self::Unavailable;
                };
                Self::TokenPaste {
                    call_id,
                    definition_id,
                    expansion_id,
                    parent_expansion_id: raw.origin.parent_expansion_id(),
                    body_token_index,
                    argument_index: raw.origin.argument_index(),
                    argument_token_index: raw.origin.argument_token_index(),
                }
            }
            Self::STRINGIFICATION => {
                let Some(call_id) = raw.origin.call_id() else {
                    return Self::Unavailable;
                };
                let Some(definition_id) = raw.origin.definition_id() else {
                    return Self::Unavailable;
                };
                let Some(expansion_id) = raw.origin.expansion_id() else {
                    return Self::Unavailable;
                };
                let Some(body_token_index) = raw.origin.body_token_index() else {
                    return Self::Unavailable;
                };
                Self::Stringify {
                    call_id,
                    definition_id,
                    expansion_id,
                    parent_expansion_id: raw.origin.parent_expansion_id(),
                    body_token_index,
                    argument_index: raw.origin.argument_index(),
                    argument_token_index: raw.origin.argument_token_index(),
                }
            }
            Self::UNAVAILABLE => Self::Unavailable,
            _ => Self::Unavailable,
        }
    }
}
