use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::provenance::builder) fn scan_references_and_state(&mut self) {
        let event_records = self.model.index.event_records.clone();
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

    pub(in crate::source::provenance::builder) fn apply_define(
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

    pub(in crate::source::provenance::builder) fn apply_undef(
        &mut self,
        source_order: usize,
        directive: &SourcePreprocEventRecord,
    ) {
        let Some(undef) = self.model.index.undefs.get(directive.index) else {
            return;
        };
        if let Some(name) = undef.name.as_ref() {
            self.current_state.remove(name.as_str());
            self.record_state_checkpoint(source_order + 1, boundary_after(directive.range));
        }
    }

    pub(in crate::source::provenance::builder) fn record_usage_reference(
        &mut self,
        directive: &SourcePreprocEventRecord,
    ) {
        let Some(usage) = self.model.index.usages.get(directive.index).cloned() else {
            return;
        };
        let Some(name) = usage.name.clone() else {
            self.record_missing_reference_name(usage.event_id);
            return;
        };
        let Some(name_range) = usage.name_range else {
            self.record_missing_reference_name_range(usage.event_id);
            return;
        };
        let event_id = usage.event_id;
        let directive_range = usage.range;
        let definition_identity = usage.definition_identity;
        let expansion_identity = usage.expansion_identity;
        let parent_expansion_identity = usage.parent_expansion_identity;
        let arguments = usage.arguments.clone();
        let resolution = self.resolve_usage_reference(name.as_str(), definition_identity);
        let reference = self.push_reference(
            event_id,
            SourceMacroReferenceSite::Usage { usage_index: directive.index },
            name.clone(),
            name_range,
            directive_range,
            resolution.clone(),
        );
        let call = self.push_call(
            reference,
            directive_range,
            resolution,
            usage.identity,
            expansion_identity,
            parent_expansion_identity,
        );
        for argument in arguments {
            self.record_macro_actual_argument(call, argument);
        }
    }

    pub(in crate::source::provenance::builder) fn record_conditional_references(
        &mut self,
        directive: &SourcePreprocEventRecord,
    ) {
        let Some(conditional) = self.model.index.conditionals.get(directive.index).cloned() else {
            return;
        };
        let event_id = conditional.event_id;
        let directive_range = conditional.range;
        for (token_index, token) in conditional.expr.iter().enumerate() {
            let name = token.value.clone();
            let Some(name_range) = token.range else {
                self.record_missing_reference_name_range(event_id);
                continue;
            };
            let (site, resolution) =
                if let Some(definition) = self.current_state.get(name.as_str()).copied() {
                    (
                        SourceMacroReferenceSite::ConditionalToken {
                            conditional_index: directive.index,
                            token_index,
                        },
                        self.resolve_definition(
                            definition,
                            SourceMacroResolutionReason::VisibleDefinition,
                        ),
                    )
                } else if let Some(definition) =
                    self.include_guard_definition_after_ifndef(directive.index, name.as_str())
                {
                    (
                        SourceMacroReferenceSite::IncludeGuardIfNDef {
                            conditional_index: directive.index,
                            token_index,
                        },
                        self.resolve_definition(
                            definition,
                            SourceMacroResolutionReason::IncludeGuardIfNDef,
                        ),
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

    pub(in crate::source::provenance::builder) fn push_reference(
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

    pub(in crate::source::provenance::builder) fn push_call(
        &mut self,
        reference: SourceMacroReferenceId,
        call_range: SourceRange,
        callee: SourceMacroResolution,
        identity: Option<MacroCallId>,
        expansion_identity: Option<MacroExpansionId>,
        parent_expansion_identity: Option<MacroExpansionId>,
    ) -> SourceMacroCallId {
        let id = SourceMacroCallId::new(self.model.macro_calls.len());
        self.model.macro_calls.push(SourceMacroCall {
            id,
            identity,
            expansion_identity,
            parent_expansion_identity,
            reference,
            call_range,
            callee,
            arguments: Vec::new(),
            expansion: None,
            status: SourceMacroCallStatus::ExpansionUnavailable(
                SourcePreprocUnavailable::MissingExpansionTokens,
            ),
        });
        if let Some(identity) = identity {
            self.call_ids_by_identity.insert(identity, id);
        } else {
            self.macro_calls_partial = true;
        }
        if let Some(expansion_identity) = expansion_identity {
            self.call_ids_by_expansion_identity.insert(expansion_identity, id);
        }
        id
    }
}
