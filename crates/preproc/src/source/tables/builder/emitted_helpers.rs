use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn call_for_emitted_token(
        &mut self,
        request: EmittedTokenMacroCall,
    ) -> Result<SourceMacroCallId, ()> {
        if let Some(call) = self.calls_by_trace_id.get(&request.trace_call).copied() {
            self.record_call_expansion_trace(
                call,
                request.trace_expansion,
                request.parent_trace_expansion,
            )?;
            return Ok(call);
        }

        let event_id = self
            .model
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
            Some(request.trace_call),
            Some(request.trace_expansion),
            request.parent_trace_expansion,
        ))
    }

    pub(in crate::source::tables::builder) fn definition_for_call(
        &self,
        call: SourceMacroCallId,
    ) -> Result<SourceMacroDefinitionId, ()> {
        let Some(call) = self.model.macro_calls.get(call) else {
            return Err(());
        };
        match &call.callee {
            SourceMacroResolution::Resolved { definition, .. } => Ok(*definition),
            SourceMacroResolution::Undefined | SourceMacroResolution::Unavailable(_) => Err(()),
        }
    }

    pub(in crate::source::tables::builder) fn definition_for_trace_id(
        &self,
        trace_definition: MacroDefinitionId,
    ) -> Result<SourceMacroDefinitionId, ()> {
        self.definitions_by_trace_id.get(&trace_definition).copied().ok_or(())
    }

    pub(in crate::source::tables::builder) fn definition_body_token_exists(
        &self,
        definition: SourceMacroDefinitionId,
        body_token_index: usize,
    ) -> bool {
        let Some(definition) = self.model.macro_definitions.get(definition) else {
            return false;
        };
        definition.body_tokens.get(body_token_index).is_some()
    }

    pub(in crate::source::tables::builder) fn definition_parameter_exists(
        &self,
        definition: SourceMacroDefinitionId,
        argument_index: usize,
    ) -> bool {
        let Some(definition) = self.model.macro_definitions.get(definition) else {
            return false;
        };
        definition.params.as_ref().is_some_and(|params| params.get(argument_index).is_some())
    }

    pub(in crate::source::tables::builder) fn record_call_expansion_trace(
        &mut self,
        call: SourceMacroCallId,
        trace_expansion: MacroExpansionId,
        parent_trace_expansion: Option<MacroExpansionId>,
    ) -> Result<(), ()> {
        let Some(call_fact) = self.model.macro_calls.get_mut(call) else {
            return Err(());
        };
        if let Some(existing) = call_fact.trace_expansion {
            if existing != trace_expansion {
                self.expansions_partial = true;
                return Err(());
            }
        } else {
            call_fact.trace_expansion = Some(trace_expansion);
            self.calls_by_expansion_trace_id.insert(trace_expansion, call);
        }
        if let Some(parent_trace_expansion) = parent_trace_expansion {
            match call_fact.parent_trace_expansion {
                Some(existing) if existing != parent_trace_expansion => {
                    self.expansions_partial = true;
                    return Err(());
                }
                Some(_) => {}
                None => call_fact.parent_trace_expansion = Some(parent_trace_expansion),
            }
        }
        Ok(())
    }

    pub(in crate::source::tables::builder) fn record_macro_argument(
        &mut self,
        call: SourceMacroCallId,
        argument_index: usize,
        argument_token_range: SourceRange,
    ) {
        let Some(call) = self.model.macro_calls.get_mut(call) else {
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

    pub(in crate::source::tables::builder) fn record_macro_actual_argument(
        &mut self,
        call: SourceMacroCallId,
        argument: SourceMacroActualArgument,
    ) {
        let Some(call) = self.model.macro_calls.get_mut(call) else {
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
