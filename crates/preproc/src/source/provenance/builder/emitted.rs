use super::{token_origin::*, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source::provenance::builder) enum MacroOperationOriginKind {
    TokenPaste,
    Stringification,
}

impl SourcePreprocModelBuilder {
    pub(in crate::source::provenance::builder) fn build_emitted_token_tables(&mut self) {
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

    pub(in crate::source::provenance::builder) fn resolve_emitted_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        token: &SourceEmittedTokenRecord,
    ) -> Option<SourceTokenOrigin> {
        match &token.origin {
            TokenOrigin::Source { token_range } => source_range_from_origin(token_range)
                .map(|token_range| SourceTokenOrigin::Source { token_range }),
            TokenOrigin::MacroBody { macro_name, identity, call_range, body_token_range } => {
                let call_range = source_range_from_origin(call_range)?;
                let body_token_range = source_range_from_origin(body_token_range)?;
                self.resolve_macro_body_token_origin(
                    token_id,
                    SmolStr::new(macro_name),
                    *identity,
                    call_range,
                    body_token_range,
                )
            }
            TokenOrigin::MacroArgument {
                identity,
                call_range,
                body_token_range,
                argument_token_range,
                ..
            } => {
                source_range_from_origin(call_range)?;
                let body_token_range = source_range_from_origin(body_token_range)?;
                let argument_token_range = source_range_from_origin(argument_token_range)?;
                self.resolve_macro_argument_token_origin(
                    token_id,
                    *identity,
                    body_token_range,
                    argument_token_range,
                )
            }
            TokenOrigin::Builtin { name, identity } if !name.is_empty() => {
                self.resolve_builtin_token_origin(token_id, SmolStr::new(name), *identity)
            }
            TokenOrigin::TokenPaste { identity } => self.resolve_macro_operation_token_origin(
                token_id,
                *identity,
                MacroOperationOriginKind::TokenPaste,
            ),
            TokenOrigin::Stringification { identity } => self.resolve_macro_operation_token_origin(
                token_id,
                *identity,
                MacroOperationOriginKind::Stringification,
            ),
            TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
        }
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_body_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        macro_name: SmolStr,
        origin: MacroBodyOrigin,
        call_range: SourceRange,
        body_token_range: SourceRange,
    ) -> Option<SourceTokenOrigin> {
        let Ok(definition) = self.definition_for_identity(origin.definition_id) else {
            return None;
        };
        let body_token_index = origin_index(origin.body_token_index)?;
        let Ok(call) = self.call_for_emitted_token(EmittedTokenMacroCall {
            token_id,
            macro_name,
            call_identity: origin.call_id,
            definition,
            call_range,
            expansion_identity: origin.expansion_id,
            parent_expansion_identity: origin.parent_expansion_id,
        }) else {
            return None;
        };

        if !self.definition_body_token_exists(definition, body_token_index) {
            return None;
        }

        self.record_emitted_token_owner(token_id, call);
        if self.source_is_predefine(body_token_range.source) {
            return Some(SourceTokenOrigin::Predefine { source: body_token_range.source });
        }
        Some(SourceTokenOrigin::MacroBody { origin, definition, body_token_range, call })
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_argument_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        origin: MacroArgumentOrigin,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    ) -> Option<SourceTokenOrigin> {
        let Ok(definition) = self.definition_for_identity(origin.definition_id) else {
            return None;
        };
        let body_token_index = origin_index(origin.body_token_index)?;
        let argument_index = origin_index(origin.argument_index)?;
        let call = self.call_ids_by_identity.get(&origin.call_id).copied()?;
        if !self.definition_body_token_exists(definition, body_token_index) {
            return None;
        }
        if !self.definition_parameter_exists(definition, argument_index) {
            return None;
        };
        self.record_macro_argument(call, argument_index, argument_token_range);
        self.record_emitted_token_owner(token_id, call);

        Some(SourceTokenOrigin::MacroArgument {
            origin,
            call,
            argument_index,
            body_token_range,
            argument_token_range,
        })
    }

    pub(in crate::source::provenance::builder) fn resolve_builtin_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        name: SmolStr,
        origin: MacroBuiltinOrigin,
    ) -> Option<SourceTokenOrigin> {
        let call = self.call_ids_by_identity.get(&origin.call_id).copied()?;
        let call_expansion_identity = origin.parent_expansion_id.unwrap_or(origin.expansion_id);
        if self.record_call_expansion_identity(call, call_expansion_identity, None).is_err() {
            return None;
        }
        self.record_emitted_token_owner(token_id, call);
        Some(SourceTokenOrigin::Builtin { name, origin, call })
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_operation_token_origin(
        &mut self,
        token_id: SourceEmittedTokenId,
        origin: MacroOperationOrigin,
        kind: MacroOperationOriginKind,
    ) -> Option<SourceTokenOrigin> {
        if self.definition_for_identity(origin.definition_id).is_err() {
            return None;
        };
        let call = self.call_ids_by_identity.get(&origin.call_id).copied()?;
        let call_expansion_identity = origin.parent_expansion_id.unwrap_or(origin.expansion_id);
        if self.record_call_expansion_identity(call, call_expansion_identity, None).is_err() {
            return None;
        }
        self.record_emitted_token_owner(token_id, call);
        match kind {
            MacroOperationOriginKind::TokenPaste => {
                Some(SourceTokenOrigin::TokenPaste { origin, call })
            }
            MacroOperationOriginKind::Stringification => {
                Some(SourceTokenOrigin::Stringification { origin, call })
            }
        }
    }
}
