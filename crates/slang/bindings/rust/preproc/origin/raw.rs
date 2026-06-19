use super::TokenOrigin;
use crate::{
    TokenKind, ffi,
    preproc::{EmittedToken, MacroCallId, MacroDefinitionId, MacroExpansionId},
};

pub(super) struct RawTokenOrigin {
    pub(super) kind: u8,
    pub(super) macro_name: String,
    pub(super) origin: RawMacroOrigin,
    pub(super) token_range: ffi::RawSourceBufferRange,
    pub(super) call_range: ffi::RawSourceBufferRange,
    pub(super) body_token_range: ffi::RawSourceBufferRange,
    pub(super) argument_token_range: ffi::RawSourceBufferRange,
}

pub(super) struct RawMacroOrigin {
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

impl EmittedToken {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawPreprocessorTraceEmittedToken) -> Self {
        let ffi::RawPreprocessorTraceEmittedToken {
            emitted_token_index,
            has_emitted_token_index,
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
            emitted_token_index: has_emitted_token_index.then_some(emitted_token_index),
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

impl RawMacroOrigin {
    #[inline]
    pub(super) fn call_id(&self) -> Option<MacroCallId> {
        self.has_call_id.then_some(MacroCallId(self.call_id))
    }

    #[inline]
    pub(super) fn definition_id(&self) -> Option<MacroDefinitionId> {
        self.has_definition_id.then_some(MacroDefinitionId(self.definition_id))
    }

    #[inline]
    pub(super) fn expansion_id(&self) -> Option<MacroExpansionId> {
        self.has_expansion_id.then_some(MacroExpansionId(self.expansion_id))
    }

    #[inline]
    pub(super) fn parent_expansion_id(&self) -> Option<MacroExpansionId> {
        self.has_parent_expansion_id.then_some(MacroExpansionId(self.parent_expansion_id))
    }

    #[inline]
    pub(super) fn body_token_index(&self) -> Option<u32> {
        self.has_body_token_index.then_some(self.body_token_index)
    }

    #[inline]
    pub(super) fn argument_index(&self) -> Option<u32> {
        self.has_argument_index.then_some(self.argument_index)
    }

    #[inline]
    pub(super) fn argument_token_index(&self) -> Option<u32> {
        self.has_argument_token_index.then_some(self.argument_token_index)
    }
}
