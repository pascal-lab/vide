use super::{token_origin::*, *};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source::provenance::builder) enum MacroOperationProvenanceKind {
    TokenPaste,
    Stringification,
}

impl SourcePreprocModelBuilder {
    pub(in crate::source::provenance::builder) fn build_emitted_token_tables(&mut self) {
        for index in 0..self.model.index.emitted_tokens.len() {
            let token = self.model.index.emitted_tokens[index].clone();
            let token_id = SourceEmittedTokenId::new(self.model.emitted_tokens.len());
            let provenance = self.resolve_emitted_token_provenance(token_id, &token);
            let provenance_id = provenance.map(|provenance| {
                let provenance_id = SourceTokenProvenanceId::new(self.model.token_provenance.len());
                self.model.token_provenance.push(provenance);
                provenance_id
            });

            self.model.emitted_tokens.push(SourceEmittedToken {
                id: token_id,
                text: token.raw,
                display: token.display,
                kind: token.kind,
                emitted_range: SourceEmittedTokenRange { start: token_id, len: 1 },
                provenance: provenance_id,
            });
        }
    }

    pub(in crate::source::provenance::builder) fn resolve_emitted_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        token: &SourceEmittedTokenRecord,
    ) -> Option<SourceTokenProvenance> {
        match &token.provenance {
            TokenOrigin::Source { token_range } => source_range_from_origin(token_range)
                .map(|token_range| SourceTokenProvenance::Source { token_range }),
            TokenOrigin::MacroBody { macro_name, identity, call_range, body_token_range } => {
                let call_range = source_range_from_origin(call_range)?;
                let body_token_range = source_range_from_origin(body_token_range)?;
                self.resolve_macro_body_token_provenance(
                    token_id,
                    SmolStr::new(macro_name),
                    macro_body_identity(identity),
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
                self.resolve_macro_argument_token_provenance(
                    token_id,
                    macro_argument_identity(identity),
                    body_token_range,
                    argument_token_range,
                )
            }
            TokenOrigin::Builtin { name, identity } if !name.is_empty() => self
                .resolve_builtin_token_provenance(
                    token_id,
                    SmolStr::new(name),
                    macro_builtin_identity(identity),
                ),
            TokenOrigin::TokenPaste { identity } => self.resolve_macro_operation_token_provenance(
                token_id,
                macro_operation_identity(identity),
                MacroOperationProvenanceKind::TokenPaste,
            ),
            TokenOrigin::Stringification { identity } => self
                .resolve_macro_operation_token_provenance(
                    token_id,
                    macro_operation_identity(identity),
                    MacroOperationProvenanceKind::Stringification,
                ),
            TokenOrigin::Builtin { .. } | TokenOrigin::Unavailable => None,
        }
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_body_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        macro_name: SmolStr,
        identity: SourceMacroBodyIdentity,
        call_range: SourceRange,
        body_token_range: SourceRange,
    ) -> Option<SourceTokenProvenance> {
        let Ok(definition) = self.definition_for_identity(identity.definition) else {
            return None;
        };
        let Ok(call) = self.call_for_emitted_token(EmittedTokenMacroCall {
            token_id,
            macro_name,
            call_identity: identity.call,
            definition,
            call_range,
            expansion_identity: identity.expansion,
            parent_expansion_identity: identity.parent_expansion,
        }) else {
            return None;
        };

        if !self.definition_body_token_exists(definition, identity.body_token_index) {
            return None;
        }

        self.record_emitted_token_owner(token_id, call);
        if self.source_is_predefine(body_token_range.source) {
            return Some(SourceTokenProvenance::Predefine { source: body_token_range.source });
        }
        Some(SourceTokenProvenance::MacroBody { identity, definition, body_token_range, call })
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_argument_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        identity: SourceMacroArgumentIdentity,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    ) -> Option<SourceTokenProvenance> {
        let Ok(definition) = self.definition_for_identity(identity.definition) else {
            return None;
        };
        let call = self.call_ids_by_identity.get(&identity.call).copied()?;
        if !self.definition_body_token_exists(definition, identity.body_token_index) {
            return None;
        }
        if !self.definition_parameter_exists(definition, identity.argument_index) {
            return None;
        };
        self.record_macro_argument(call, identity.argument_index, argument_token_range);
        self.record_emitted_token_owner(token_id, call);

        Some(SourceTokenProvenance::MacroArgument {
            identity,
            call,
            argument_index: identity.argument_index,
            body_token_range,
            argument_token_range,
        })
    }

    pub(in crate::source::provenance::builder) fn resolve_builtin_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        name: SmolStr,
        identity: SourceMacroBuiltinIdentity,
    ) -> Option<SourceTokenProvenance> {
        let call = self.call_ids_by_identity.get(&identity.call).copied()?;
        let call_expansion_identity = identity.parent_expansion.unwrap_or(identity.expansion);
        if self.record_call_expansion_identity(call, call_expansion_identity, None).is_err() {
            return None;
        }
        self.record_emitted_token_owner(token_id, call);
        Some(SourceTokenProvenance::Builtin { name, identity, call })
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_operation_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        identity: SourceMacroOperationIdentity,
        kind: MacroOperationProvenanceKind,
    ) -> Option<SourceTokenProvenance> {
        if self.definition_for_identity(identity.definition).is_err() {
            return None;
        };
        let call = self.call_ids_by_identity.get(&identity.call).copied()?;
        let call_expansion_identity = identity.parent_expansion.unwrap_or(identity.expansion);
        if self.record_call_expansion_identity(call, call_expansion_identity, None).is_err() {
            return None;
        }
        self.record_emitted_token_owner(token_id, call);
        match kind {
            MacroOperationProvenanceKind::TokenPaste => {
                Some(SourceTokenProvenance::TokenPaste { identity, call })
            }
            MacroOperationProvenanceKind::Stringification => {
                Some(SourceTokenProvenance::Stringification { identity, call })
            }
        }
    }
}
