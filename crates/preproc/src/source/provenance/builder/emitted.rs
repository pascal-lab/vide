use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::source::provenance::builder) enum MacroOperationProvenanceKind {
    TokenPaste,
    Stringification,
}

impl<'a> SourcePreprocModelBuilder<'a> {
    pub(in crate::source::provenance::builder) fn build_emitted_token_tables(&mut self) {
        for index in 0..self.index.emitted_tokens.len() {
            let token = self.index.emitted_tokens[index].clone();
            let token_id = SourceEmittedTokenId::new(self.tables.emitted_tokens.len());
            let provenance = self.resolve_emitted_token_provenance(token_id, &token);
            let provenance_id = SourceTokenProvenanceId::new(self.tables.token_provenance.len());
            self.tables.token_provenance.push(provenance);

            self.tables.emitted_tokens.push(SourceEmittedToken {
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
        token: &SourceEmittedTokenFact,
    ) -> SourceTokenProvenance {
        match &token.provenance {
            SourceTokenProvenanceFact::Source { token_range } => {
                SourceTokenProvenance::Source { token_range: *token_range }
            }
            SourceTokenProvenanceFact::MacroBody {
                macro_name,
                identity,
                call_range,
                body_token_range,
            } => self.resolve_macro_body_token_provenance(
                token_id,
                macro_name.clone(),
                *identity,
                *call_range,
                *body_token_range,
            ),
            SourceTokenProvenanceFact::MacroArgument {
                identity,
                body_token_range,
                argument_token_range,
                ..
            } => self.resolve_macro_argument_token_provenance(
                token_id,
                *identity,
                *body_token_range,
                *argument_token_range,
            ),
            SourceTokenProvenanceFact::Builtin { name, identity } if !name.is_empty() => {
                self.resolve_builtin_token_provenance(token_id, name.clone(), *identity)
            }
            SourceTokenProvenanceFact::TokenPaste { identity } => self
                .resolve_macro_operation_token_provenance(
                    token_id,
                    *identity,
                    MacroOperationProvenanceKind::TokenPaste,
                ),
            SourceTokenProvenanceFact::Stringification { identity } => self
                .resolve_macro_operation_token_provenance(
                    token_id,
                    *identity,
                    MacroOperationProvenanceKind::Stringification,
                ),
            SourceTokenProvenanceFact::Builtin { .. } | SourceTokenProvenanceFact::Unavailable => {
                self.unavailable_token_provenance()
            }
        }
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_body_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        macro_name: SmolStr,
        identity: Option<SourceMacroBodyIdentity>,
        call_range: SourceRange,
        body_token_range: SourceRange,
    ) -> SourceTokenProvenance {
        let Some(identity) = identity else {
            return self.unavailable_token_provenance();
        };
        let Ok(definition) = self.definition_for_identity(identity.definition) else {
            return self.unavailable_token_provenance();
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
            return self.unavailable_token_provenance();
        };

        if !self.definition_body_token_exists(definition, identity.body_token_index) {
            return self.unavailable_token_provenance();
        }

        self.record_emitted_token_owner(token_id, call);
        if self.source_is_predefine(body_token_range.source) {
            return SourceTokenProvenance::Predefine { source: body_token_range.source };
        }
        SourceTokenProvenance::MacroBody { identity, definition, body_token_range, call }
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_argument_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        identity: Option<SourceMacroArgumentIdentity>,
        body_token_range: SourceRange,
        argument_token_range: SourceRange,
    ) -> SourceTokenProvenance {
        let Some(identity) = identity else {
            return self.unavailable_token_provenance();
        };
        let Ok(definition) = self.definition_for_identity(identity.definition) else {
            return self.unavailable_token_provenance();
        };
        let Some(call) = self.call_ids_by_identity.get(&identity.call).copied() else {
            return self.unavailable_token_provenance();
        };
        if !self.definition_body_token_exists(definition, identity.body_token_index) {
            return self.unavailable_token_provenance();
        }
        if !self.definition_parameter_exists(definition, identity.argument_index) {
            return self.unavailable_token_provenance();
        };
        self.record_macro_argument(call, identity.argument_index, argument_token_range);
        self.record_emitted_token_owner(token_id, call);

        SourceTokenProvenance::MacroArgument {
            identity,
            call,
            argument_index: identity.argument_index,
            body_token_range,
            argument_token_range,
        }
    }

    pub(in crate::source::provenance::builder) fn resolve_builtin_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        name: SmolStr,
        identity: Option<SourceMacroBuiltinIdentity>,
    ) -> SourceTokenProvenance {
        let Some(identity) = identity else {
            return self.unavailable_token_provenance();
        };
        let Some(call) = self.call_ids_by_identity.get(&identity.call).copied() else {
            return self.unavailable_token_provenance();
        };
        let call_expansion_identity = identity.parent_expansion.unwrap_or(identity.expansion);
        if self.record_call_expansion_identity(call, call_expansion_identity, None).is_err() {
            return self.unavailable_token_provenance();
        }
        self.record_emitted_token_owner(token_id, call);
        SourceTokenProvenance::Builtin { name, identity, call }
    }

    pub(in crate::source::provenance::builder) fn resolve_macro_operation_token_provenance(
        &mut self,
        token_id: SourceEmittedTokenId,
        identity: Option<SourceMacroOperationIdentity>,
        kind: MacroOperationProvenanceKind,
    ) -> SourceTokenProvenance {
        let Some(identity) = identity else {
            return self.unavailable_token_provenance();
        };
        if self.definition_for_identity(identity.definition).is_err() {
            return self.unavailable_token_provenance();
        };
        let Some(call) = self.call_ids_by_identity.get(&identity.call).copied() else {
            return self.unavailable_token_provenance();
        };
        let call_expansion_identity = identity.parent_expansion.unwrap_or(identity.expansion);
        if self.record_call_expansion_identity(call, call_expansion_identity, None).is_err() {
            return self.unavailable_token_provenance();
        }
        self.record_emitted_token_owner(token_id, call);
        match kind {
            MacroOperationProvenanceKind::TokenPaste => {
                SourceTokenProvenance::TokenPaste { identity, call }
            }
            MacroOperationProvenanceKind::Stringification => {
                SourceTokenProvenance::Stringification { identity, call }
            }
        }
    }
}
