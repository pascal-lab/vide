use super::{emitted_origins::*, token_origin::*, *};

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn build_emitted_token_tables(&mut self) {
        for index in 0..self.model.index.emitted_tokens.len() {
            let token = self.model.index.emitted_tokens[index].clone();
            let token_id = SourceEmittedTokenId::new(self.model.emitted_tokens.len());
            let origin = self.resolve_emitted_token_origin(token_id, &token);
            let origin_id = origin.map(|origin| {
                let origin_id = SourceTokenOriginId::new(self.model.token_origins.len());
                self.model.token_origins.push(origin);
                origin_id
            });

            self.model.emitted_tokens.push(SourceEmittedToken {
                id: token_id,
                text: token.raw,
                display: token.display,
                kind: token.kind,
                emitted_range: SourceEmittedTokenRange { start: token_id, len: 1 },
                origin: origin_id,
            });
        }
    }

    pub(in crate::source::tables::builder) fn resolve_emitted_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        token: &SourceEmittedTokenRecord,
    ) -> Option<SourceTokenOrigin> {
        match &token.origin {
            TokenOrigin::Source { token_range } => source_range_from_origin(token_range)
                .map(|token_range| SourceTokenOrigin::Source { token_range }),
            TokenOrigin::MacroBody {
                macro_name,
                call_id,
                definition_id,
                expansion_id,
                parent_expansion_id,
                body_token_index,
                call_range,
                body_token_range,
            } => {
                let call_range = source_range_from_origin(call_range)?;
                let body_token_range = source_range_from_origin(body_token_range)?;
                self.resolve_macro_body_token_origin(MacroBodyOriginInput {
                    token_id,
                    macro_name: SmolStr::new(macro_name),
                    trace_call: *call_id,
                    trace_definition: *definition_id,
                    trace_expansion: *expansion_id,
                    parent_trace_expansion: *parent_expansion_id,
                    body_token_index: *body_token_index,
                    call_range,
                    body_token_range,
                })
            }
            TokenOrigin::MacroArgument {
                call_id,
                definition_id,
                body_token_index,
                argument_index,
                argument_token_index,
                call_range,
                body_token_range,
                argument_token_range,
                ..
            } => {
                source_range_from_origin(call_range)?;
                let body_token_range = source_range_from_origin(body_token_range)?;
                let argument_token_range = source_range_from_origin(argument_token_range)?;
                self.resolve_macro_argument_token_origin(MacroArgumentOriginInput {
                    token_id,
                    trace_call: *call_id,
                    trace_definition: *definition_id,
                    body_token_index: *body_token_index,
                    trace_argument_index: *argument_index,
                    argument_token_index: *argument_token_index,
                    body_token_range,
                    argument_token_range,
                })
            }
            TokenOrigin::Builtin { name, call_id, expansion_id, parent_expansion_id }
                if !name.is_empty() =>
            {
                self.resolve_builtin_token_origin(
                    token_id,
                    SmolStr::new(name),
                    *call_id,
                    *expansion_id,
                    *parent_expansion_id,
                )
            }
            TokenOrigin::TokenPaste {
                call_id,
                definition_id,
                expansion_id,
                parent_expansion_id,
                argument_index,
                argument_token_index,
                ..
            } => self.resolve_macro_operation_token_origin(MacroOperationOriginInput {
                token_id,
                trace_call: *call_id,
                trace_definition: *definition_id,
                trace_expansion: *expansion_id,
                parent_trace_expansion: *parent_expansion_id,
                argument_index: *argument_index,
                argument_token_index: *argument_token_index,
                kind: MacroOperationKind::TokenPaste,
            }),
            TokenOrigin::Stringify {
                call_id,
                definition_id,
                expansion_id,
                parent_expansion_id,
                argument_index,
                argument_token_index,
                ..
            } => self.resolve_macro_operation_token_origin(MacroOperationOriginInput {
                token_id,
                trace_call: *call_id,
                trace_definition: *definition_id,
                trace_expansion: *expansion_id,
                parent_trace_expansion: *parent_expansion_id,
                argument_index: *argument_index,
                argument_token_index: *argument_token_index,
                kind: MacroOperationKind::Stringify,
            }),
            TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
        }
    }
}
