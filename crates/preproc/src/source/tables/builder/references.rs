use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn scan_references_and_state(&mut self) {
        let event_records = self.event_records.clone();
        for (source_order, directive) in event_records.iter().enumerate() {
            match directive.kind {
                MacroEventKind::Define => self.apply_define(source_order, directive),
                MacroEventKind::Undef => self.apply_undef(source_order, directive),
                MacroEventKind::Conditional => self.record_conditional_references(directive),
                MacroEventKind::Usage => self.record_usage_reference(directive),
                MacroEventKind::Include | MacroEventKind::Branch => {}
            }
        }
    }

    pub(in crate::source::tables::builder) fn apply_define(
        &mut self,
        source_order: usize,
        directive: &SourcePreprocEventRecord,
    ) {
        if let Some(definition_id) = self.definition_ids_by_define_index.get(&directive.index) {
            let definition = self
                .model
                .macro_definitions
                .get(*definition_id)
                .expect("definition id should point at inserted definition");
            self.current_state.insert(definition.name.clone(), *definition_id);
            self.record_state_checkpoint(source_order + 1, boundary_after(directive.range));
        }
    }

    pub(in crate::source::tables::builder) fn apply_undef(
        &mut self,
        source_order: usize,
        directive: &SourcePreprocEventRecord,
    ) {
        let Some(undef) = self.undefs.get(directive.index) else {
            return;
        };
        if let Some(name) = undef.name.as_ref() {
            self.current_state.remove(name.as_str());
            self.record_state_checkpoint(source_order + 1, boundary_after(directive.range));
        }
    }

    pub(in crate::source::tables::builder) fn record_usage_reference(
        &mut self,
        directive: &SourcePreprocEventRecord,
    ) {
        let Some(usage) = self.usages.get(directive.index).cloned() else {
            return;
        };
        let Some(name) = usage.name.clone() else {
            return;
        };
        let Some(name_range) = usage.name_range else {
            return;
        };
        let event_id = usage.event_id;
        let directive_range = usage.range;
        let trace_definition = usage.trace_definition;
        let arguments = usage.arguments.clone();
        let resolution = self.resolve_usage_reference(name.as_str(), trace_definition);
        let reference = self.push_reference(
            event_id,
            SourceMacroReferenceSite::Usage { usage_index: directive.index },
            name.clone(),
            name_range,
            directive_range,
            resolution.clone(),
        );
        let call = self.push_call(reference, directive_range, usage.trace_call);
        for argument in arguments {
            self.record_macro_actual_argument(call, argument);
        }
    }

    pub(in crate::source::tables::builder) fn record_conditional_references(
        &mut self,
        directive: &SourcePreprocEventRecord,
    ) {
        let Some(conditional) = self.conditionals.get(directive.index).cloned() else {
            return;
        };
        let event_id = conditional.event_id;
        let directive_range = conditional.range;
        for (token_index, token) in conditional.expr.iter().enumerate() {
            let name = token.value.clone();
            let Some(name_range) = token.range else {
                continue;
            };
            let (site, resolution) =
                if let Some(definition) = self.current_state.get(name.as_str()).copied() {
                    (
                        SourceMacroReferenceSite::ConditionalToken {
                            conditional_index: directive.index,
                            token_index,
                        },
                        self.resolve_definition(definition),
                    )
                } else if let Some(definition) =
                    self.include_guard_definition_after_ifndef(directive.index, name.as_str())
                {
                    (
                        SourceMacroReferenceSite::IncludeGuardIfNDef {
                            conditional_index: directive.index,
                            token_index,
                        },
                        self.resolve_definition(definition),
                    )
                } else {
                    (
                        SourceMacroReferenceSite::ConditionalToken {
                            conditional_index: directive.index,
                            token_index,
                        },
                        SourceMacroResolution::Undefined,
                    )
                };
            self.push_reference(event_id, site, name, name_range, directive_range, resolution);
        }
    }

    pub(in crate::source::tables::builder) fn push_reference(
        &mut self,
        event_id: SourcePreprocEventId,
        site: SourceMacroReferenceSite,
        name: SmolStr,
        name_range: SourceRange,
        directive_range: SourceRange,
        resolution: SourceMacroResolution,
    ) -> SourceMacroReferenceId {
        let id = SourceMacroReferenceId::new(self.model.macro_references.len());
        self.model.macro_references.push(SourceMacroReference {
            id,
            event_id,
            site,
            name,
            name_range,
            directive_range,
            resolution,
        });
        id
    }

    pub(in crate::source::tables::builder) fn push_call(
        &mut self,
        reference: SourceMacroReferenceId,
        call_range: SourceRange,
        trace_call: Option<MacroCallId>,
    ) -> SourceMacroCallId {
        let id = SourceMacroCallId::new(self.model.macro_calls.len());
        self.model.macro_calls.push(SourceMacroCall {
            id,
            trace_call,
            reference,
            call_range,
            arguments: Vec::new(),
        });
        if let Some(trace_call) = trace_call {
            self.calls_by_trace_id.insert(trace_call, id);
        }
        id
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
    fn merge_optional_same_source(self, next: Option<SourceRange>) -> Option<SourceRange>;
}

impl SourceRangeOptionExt for Option<SourceRange> {
    fn merge_optional_same_source(self, next: Option<SourceRange>) -> Option<SourceRange> {
        match next {
            Some(next) => self.merge_same_source(next),
            None => self,
        }
    }
}

trait SourceRangeExt {
    fn merge_same_source(self, next: SourceRange) -> Option<SourceRange>;
}

impl SourceRangeExt for Option<SourceRange> {
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
}
