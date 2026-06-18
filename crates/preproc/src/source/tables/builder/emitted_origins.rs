use super::{token_origin::origin_index, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source::tables::builder) enum MacroOperationKind {
    TokenPaste,
    Stringify,
}

pub(in crate::source::tables::builder) struct MacroBodyOriginInput {
    pub(in crate::source::tables::builder) token_id: SourceEmittedTokenId,
    pub(in crate::source::tables::builder) macro_name: SmolStr,
    pub(in crate::source::tables::builder) trace_call: MacroCallId,
    pub(in crate::source::tables::builder) trace_definition: MacroDefinitionId,
    pub(in crate::source::tables::builder) trace_expansion: MacroExpansionId,
    pub(in crate::source::tables::builder) parent_trace_expansion: Option<MacroExpansionId>,
    pub(in crate::source::tables::builder) body_token_index: u32,
    pub(in crate::source::tables::builder) call_range: SourceRange,
    pub(in crate::source::tables::builder) body_token_range: SourceRange,
}

pub(in crate::source::tables::builder) struct MacroArgumentOriginInput {
    pub(in crate::source::tables::builder) token_id: SourceEmittedTokenId,
    pub(in crate::source::tables::builder) trace_call: MacroCallId,
    pub(in crate::source::tables::builder) trace_definition: MacroDefinitionId,
    pub(in crate::source::tables::builder) body_token_index: u32,
    pub(in crate::source::tables::builder) trace_argument_index: u32,
    pub(in crate::source::tables::builder) argument_token_index: u32,
    pub(in crate::source::tables::builder) body_token_range: SourceRange,
    pub(in crate::source::tables::builder) argument_token_range: SourceRange,
}

pub(in crate::source::tables::builder) struct MacroOperationOriginInput {
    pub(in crate::source::tables::builder) token_id: SourceEmittedTokenId,
    pub(in crate::source::tables::builder) trace_call: MacroCallId,
    pub(in crate::source::tables::builder) trace_definition: MacroDefinitionId,
    pub(in crate::source::tables::builder) trace_expansion: MacroExpansionId,
    pub(in crate::source::tables::builder) parent_trace_expansion: Option<MacroExpansionId>,
    pub(in crate::source::tables::builder) argument_index: Option<u32>,
    pub(in crate::source::tables::builder) argument_token_index: Option<u32>,
    pub(in crate::source::tables::builder) kind: MacroOperationKind,
}

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn resolve_macro_body_token_origin(
        &mut self,
        input: MacroBodyOriginInput,
    ) -> Option<SourceTokenOrigin> {
        let MacroBodyOriginInput {
            token_id,
            macro_name,
            trace_call,
            trace_definition,
            trace_expansion,
            parent_trace_expansion,
            body_token_index,
            call_range,
            body_token_range,
        } = input;
        let Ok(definition) = self.definition_for_trace_id(trace_definition) else {
            return None;
        };
        let body_token = origin_index(body_token_index)?;
        let Ok(call) = self.call_for_emitted_token(EmittedTokenMacroCall {
            token_id,
            macro_name,
            trace_call,
            definition,
            call_range,
            trace_expansion,
            parent_trace_expansion,
        }) else {
            return None;
        };

        if !self.definition_body_token_exists(definition, body_token) {
            return None;
        }

        self.record_emitted_token_owner(token_id, call);
        if self.source_is_predefine(body_token_range.source) {
            return Some(SourceTokenOrigin::Predefine { source: body_token_range.source });
        }
        Some(SourceTokenOrigin::MacroBody {
            trace_call,
            trace_definition,
            definition,
            body_token_range,
            call,
        })
    }

    pub(in crate::source::tables::builder) fn resolve_macro_argument_token_origin(
        &mut self,
        input: MacroArgumentOriginInput,
    ) -> Option<SourceTokenOrigin> {
        let MacroArgumentOriginInput {
            token_id,
            trace_call,
            trace_definition,
            body_token_index,
            trace_argument_index,
            argument_token_index,
            body_token_range,
            argument_token_range,
        } = input;
        let Ok(definition) = self.definition_for_trace_id(trace_definition) else {
            return None;
        };
        let body_token = origin_index(body_token_index)?;
        let argument_index = origin_index(trace_argument_index)?;
        let call = self.calls_by_trace_id.get(&trace_call).copied()?;
        if !self.definition_body_token_exists(definition, body_token) {
            return None;
        }
        if !self.definition_parameter_exists(definition, argument_index) {
            return None;
        };
        self.record_macro_argument(call, argument_index, argument_token_range);
        self.record_emitted_token_owner(token_id, call);

        Some(SourceTokenOrigin::MacroArgument {
            trace_call,
            argument_token_index,
            call,
            argument_index,
            body_token_range,
            argument_token_range,
        })
    }

    pub(in crate::source::tables::builder) fn resolve_builtin_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        name: SmolStr,
        trace_call: MacroCallId,
        trace_expansion: MacroExpansionId,
        parent_trace_expansion: Option<MacroExpansionId>,
    ) -> Option<SourceTokenOrigin> {
        let call = self.calls_by_trace_id.get(&trace_call).copied()?;
        let call_trace_expansion = parent_trace_expansion.unwrap_or(trace_expansion);
        if self.record_call_expansion_trace(call, call_trace_expansion, None).is_err() {
            return None;
        }
        self.record_emitted_token_owner(token_id, call);
        Some(SourceTokenOrigin::Builtin { name, trace_call, call })
    }

    pub(in crate::source::tables::builder) fn resolve_macro_operation_token_origin(
        &mut self,
        input: MacroOperationOriginInput,
    ) -> Option<SourceTokenOrigin> {
        let MacroOperationOriginInput {
            token_id,
            trace_call,
            trace_definition,
            trace_expansion,
            parent_trace_expansion,
            argument_index,
            argument_token_index,
            kind,
        } = input;
        if self.definition_for_trace_id(trace_definition).is_err() {
            return None;
        };
        let call = self.calls_by_trace_id.get(&trace_call).copied()?;
        let call_trace_expansion = parent_trace_expansion.unwrap_or(trace_expansion);
        if self.record_call_expansion_trace(call, call_trace_expansion, None).is_err() {
            return None;
        }
        self.record_emitted_token_owner(token_id, call);
        match kind {
            MacroOperationKind::TokenPaste => Some(SourceTokenOrigin::TokenPaste {
                trace_call,
                argument_index,
                argument_token_index,
                call,
            }),
            MacroOperationKind::Stringify => Some(SourceTokenOrigin::Stringify {
                trace_call,
                argument_index,
                argument_token_index,
                call,
            }),
        }
    }
}
