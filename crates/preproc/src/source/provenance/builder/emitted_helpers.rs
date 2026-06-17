use super::*;

impl<'a> SourcePreprocModelBuilder<'a> {
    pub(in crate::source::provenance::builder) fn call_for_emitted_token(
        &mut self,
        request: EmittedTokenMacroCall,
    ) -> Result<SourceMacroCallId, ()> {
        if let Some(call) = self.call_ids_by_identity.get(&request.call_identity).copied() {
            self.record_call_expansion_identity(
                call,
                request.expansion_identity,
                request.parent_expansion_identity,
            )?;
            return Ok(call);
        }

        let event_id = self
            .tables
            .macro_definitions
            .get(request.definition)
            .expect("definition id should point at inserted definition")
            .event_id;
        let resolution = self
            .resolve_definition(request.definition, SourceMacroResolutionReason::VisibleDefinition);
        let reference = self.push_reference(
            event_id,
            SourceMacroReferenceSite::ExpansionToken { emitted_token: request.token_id },
            request.macro_name.clone(),
            request.call_range,
            request.call_range,
            resolution.clone(),
        );
        Ok(self.push_call(
            reference,
            request.call_range,
            resolution,
            Some(request.call_identity),
            Some(request.expansion_identity),
            request.parent_expansion_identity,
        ))
    }

    pub(in crate::source::provenance::builder) fn definition_for_call(
        &self,
        call: SourceMacroCallId,
    ) -> Result<SourceMacroDefinitionId, ()> {
        let Some(call) = self.tables.macro_calls.get(call) else {
            return Err(());
        };
        match &call.callee {
            SourceMacroResolution::Resolved { definition, .. } => Ok(*definition),
            SourceMacroResolution::Undefined | SourceMacroResolution::Unavailable(_) => Err(()),
        }
    }

    pub(in crate::source::provenance::builder) fn definition_for_identity(
        &self,
        identity: SourceMacroDefinitionKey,
    ) -> Result<SourceMacroDefinitionId, ()> {
        self.definition_ids_by_identity.get(&identity).copied().ok_or(())
    }

    pub(in crate::source::provenance::builder) fn definition_body_token_exists(
        &self,
        definition: SourceMacroDefinitionId,
        body_token_index: usize,
    ) -> bool {
        let Some(definition) = self.tables.macro_definitions.get(definition) else {
            return false;
        };
        definition.body_tokens.get(body_token_index).is_some()
    }

    pub(in crate::source::provenance::builder) fn definition_parameter_exists(
        &self,
        definition: SourceMacroDefinitionId,
        argument_index: usize,
    ) -> bool {
        let Some(definition) = self.tables.macro_definitions.get(definition) else {
            return false;
        };
        definition.params.as_ref().is_some_and(|params| params.get(argument_index).is_some())
    }

    pub(in crate::source::provenance::builder) fn record_call_expansion_identity(
        &mut self,
        call: SourceMacroCallId,
        expansion_identity: SourceMacroExpansionKey,
        parent_expansion_identity: Option<SourceMacroExpansionKey>,
    ) -> Result<(), ()> {
        let Some(call_fact) = self.tables.macro_calls.get_mut(call) else {
            return Err(());
        };
        if let Some(existing) = call_fact.expansion_identity {
            if existing != expansion_identity {
                self.expansions_partial = true;
                return Err(());
            }
        } else {
            call_fact.expansion_identity = Some(expansion_identity);
            self.call_ids_by_expansion_identity.insert(expansion_identity, call);
        }
        if let Some(parent_expansion_identity) = parent_expansion_identity {
            match call_fact.parent_expansion_identity {
                Some(existing) if existing != parent_expansion_identity => {
                    self.expansions_partial = true;
                    return Err(());
                }
                Some(_) => {}
                None => call_fact.parent_expansion_identity = Some(parent_expansion_identity),
            }
        }
        Ok(())
    }

    pub(in crate::source::provenance::builder) fn record_macro_argument(
        &mut self,
        call: SourceMacroCallId,
        argument_index: usize,
        argument_token_range: SourceRange,
    ) {
        let Some(call) = self.tables.macro_calls.get_mut(call) else {
            return;
        };
        if let Some(argument) =
            call.arguments.iter_mut().find(|argument| argument.argument_index == argument_index)
        {
            argument.argument_range =
                argument.argument_range.merge_same_source(argument_token_range);
            return;
        }
        call.arguments.push(SourceMacroArgument {
            argument_index,
            argument_range: Some(argument_token_range),
            tokens: Vec::new(),
        });
        call.arguments.sort_by_key(|argument| argument.argument_index);
    }

    pub(in crate::source::provenance::builder) fn record_macro_actual_argument(
        &mut self,
        call: SourceMacroCallId,
        argument: SourceMacroActualArgument,
    ) {
        let Some(call) = self.tables.macro_calls.get_mut(call) else {
            return;
        };
        if let Some(existing) = call
            .arguments
            .iter_mut()
            .find(|existing| existing.argument_index == argument.argument_index)
        {
            existing.argument_range =
                existing.argument_range.merge_optional_same_source(argument.argument_range);
            if existing.tokens.is_empty() {
                existing.tokens = argument.tokens;
            }
            return;
        }
        call.arguments.push(SourceMacroArgument {
            argument_index: argument.argument_index,
            argument_range: argument.argument_range,
            tokens: argument.tokens,
        });
        call.arguments.sort_by_key(|argument| argument.argument_index);
    }
}

trait SourceRangeOptionExt {
    fn merge_same_source(self, next: SourceRange) -> Option<SourceRange>;
    fn merge_optional_same_source(self, next: Option<SourceRange>) -> Option<SourceRange>;
}

impl SourceRangeOptionExt for Option<SourceRange> {
    fn merge_same_source(self, next: SourceRange) -> Option<SourceRange> {
        let Some(existing) = self else {
            return Some(next);
        };
        if existing.source != next.source {
            return Some(existing);
        }
        Some(SourceRange {
            source: existing.source,
            range: utils::line_index::TextRange::new(
                existing.range.start().min(next.range.start()),
                existing.range.end().max(next.range.end()),
            ),
        })
    }

    fn merge_optional_same_source(self, next: Option<SourceRange>) -> Option<SourceRange> {
        match next {
            Some(next) => self.merge_same_source(next),
            None => self,
        }
    }
}
