use super::*;

impl<'a> SourcePreprocModelBuilder<'a> {
    pub(in crate::source::provenance::builder) fn resolve_visible_reference(
        &mut self,
        name: &str,
    ) -> SourceMacroResolution {
        let Some(definition) = self.current_state.get(name).copied() else {
            return SourceMacroResolution::Undefined;
        };
        self.resolve_definition(definition, SourceMacroResolutionReason::VisibleDefinition)
    }

    pub(in crate::source::provenance::builder) fn resolve_usage_reference(
        &mut self,
        name: &str,
        identity: Option<MacroDefinitionId>,
    ) -> SourceMacroResolution {
        let Some(identity) = identity else {
            return self.resolve_visible_reference(name);
        };
        let Some(definition) = self.definition_ids_by_identity.get(&identity).copied() else {
            self.references_partial = true;
            return SourceMacroResolution::Unavailable(
                SourcePreprocUnavailable::UnknownMacroUsageDefinitionIdentity { identity },
            );
        };
        self.resolve_definition(definition, SourceMacroResolutionReason::VisibleDefinition)
    }

    pub(in crate::source::provenance::builder) fn resolve_visible_reference_at_position(
        &mut self,
        name: &str,
        position: SourcePosition,
    ) -> SourceMacroResolution {
        let Some(definition) = self
            .tables
            .state_timeline
            .state_at_position(position)
            .and_then(|state| state.definitions.get(name).copied())
        else {
            return SourceMacroResolution::Undefined;
        };
        self.resolve_definition(definition, SourceMacroResolutionReason::VisibleDefinition)
    }

    pub(in crate::source::provenance::builder) fn resolve_definition(
        &mut self,
        definition: SourceMacroDefinitionId,
        reason: SourceMacroResolutionReason,
    ) -> SourceMacroResolution {
        let definition_source = self
            .tables
            .macro_definitions
            .get(definition)
            .expect("definition id should point at inserted definition")
            .directive_range
            .source;
        match self.include_chain_for_source(definition_source) {
            Ok(include_chain) => {
                SourceMacroResolution::Resolved { definition, reason, include_chain }
            }
            Err(source) => {
                self.references_partial = true;
                self.tables.issues.push(SourcePreprocFactIssue::DetachedSource { source });
                SourceMacroResolution::Unavailable(SourcePreprocUnavailable::DetachedSource {
                    source,
                })
            }
        }
    }

    pub(in crate::source::provenance::builder) fn include_chain_for_source(
        &self,
        source: PreprocSourceId,
    ) -> Result<Vec<SourceIncludeChainEntry>, PreprocSourceId> {
        let mut chain = Vec::new();
        let mut current = source;

        loop {
            let source = self
                .index
                .sources
                .iter()
                .find(|candidate| candidate.id == current)
                .expect("source id should point at an indexed preprocessor source");

            match source.origin {
                PreprocSourceOrigin::Root | PreprocSourceOrigin::Predefine => break,
                PreprocSourceOrigin::Detached => {
                    return Err(current);
                }
                PreprocSourceOrigin::Included { include_event_id } => {
                    let directive = self
                        .tables
                        .include_graph
                        .directives()
                        .iter()
                        .find(|directive| directive.event_id == include_event_id)
                        .expect("included source should point at an include directive");
                    chain.push(SourceIncludeChainEntry {
                        include_event_id,
                        include_range: directive.directive_range,
                        included_source: current,
                    });
                    current = directive.directive_range.source;
                }
            }
        }

        chain.reverse();
        Ok(chain)
    }

    pub(in crate::source::provenance::builder) fn include_guard_definition_after_ifndef(
        &self,
        conditional_index: usize,
        name: &str,
    ) -> Option<SourceMacroDefinitionId> {
        let conditional = self.index.conditionals.get(conditional_index)?;
        if conditional.kind != MacroConditionalKind::IfNDef {
            return None;
        }

        let source = conditional.range.source;
        let (conditional_order, _) =
            self.index.event_records.iter().enumerate().find(|(_, directive)| {
                directive.kind == MacroEventKind::Conditional
                    && directive.index == conditional_index
            })?;
        for directive in self.index.event_records.iter().skip(conditional_order + 1) {
            if directive.range.source != source {
                continue;
            }
            match directive.kind {
                MacroEventKind::Define => {
                    let define = self.index.defines.get(directive.index)?;
                    if define.name.as_deref() == Some(name) {
                        return self.definition_ids_by_define_index.get(&directive.index).copied();
                    }
                }
                MacroEventKind::Branch => break,
                MacroEventKind::Undef
                | MacroEventKind::Include
                | MacroEventKind::Conditional
                | MacroEventKind::Usage => {}
            }
        }
        None
    }

    pub(in crate::source::provenance::builder) fn record_missing_reference_name(
        &mut self,
        event_id: SourcePreprocEventId,
    ) {
        self.references_partial = true;
        self.tables.issues.push(SourcePreprocFactIssue::MissingReferenceName { event_id });
    }

    pub(in crate::source::provenance::builder) fn record_missing_reference_name_range(
        &mut self,
        event_id: SourcePreprocEventId,
    ) {
        self.references_partial = true;
        self.tables.issues.push(SourcePreprocFactIssue::MissingReferenceNameRange { event_id });
    }

    pub(in crate::source::provenance::builder) fn record_state_checkpoint(
        &mut self,
        source_order: usize,
        boundary: SourcePosition,
    ) {
        let id = SourceMacroStateId::new(self.tables.state_timeline.states.len());
        self.tables
            .state_timeline
            .states
            .push(SourceMacroState { id, definitions: self.current_state.clone() });
        self.tables.state_timeline.checkpoints.push(SourceMacroStateCheckpoint {
            source_order,
            boundary,
            state: id,
        });
    }
}
