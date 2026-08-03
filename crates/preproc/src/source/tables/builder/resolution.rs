use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::tables::builder) fn resolve_visible_reference(
        &mut self,
        name: &str,
    ) -> SourceMacroResolution {
        let Some(definition) = self.current_state.get(name).copied() else {
            return SourceMacroResolution::Undefined;
        };
        self.resolve_definition(definition)
    }

    pub(in crate::source::tables::builder) fn resolve_usage_reference(
        &mut self,
        name: &str,
        definition_id: Option<MacroDefinitionId>,
    ) -> SourceMacroResolution {
        let Some(definition_id) = definition_id else {
            return self.resolve_visible_reference(name);
        };
        let Some(definition) = self.definitions_by_trace_id.get(&definition_id).copied() else {
            return SourceMacroResolution::Unavailable(
                SourcePreprocUnavailable::UnknownMacroUsageDefinition { definition: definition_id },
            );
        };
        self.resolve_definition(definition)
    }

    pub(in crate::source::tables::builder) fn resolve_visible_reference_at_position(
        &mut self,
        name: &str,
        position: SourcePosition,
    ) -> SourceMacroResolution {
        let Some(definition) = self
            .model
            .state_timeline
            .state_at_position(position)
            .and_then(|state| state.definitions.get(name).copied())
        else {
            return SourceMacroResolution::Undefined;
        };
        self.resolve_definition(definition)
    }

    pub(in crate::source::tables::builder) fn resolve_definition(
        &mut self,
        definition: SourceMacroDefinitionId,
    ) -> SourceMacroResolution {
        let definition_source = self
            .model
            .macro_definitions
            .get(definition)
            .expect("definition id should point at inserted definition")
            .directive_range
            .source;
        match self.source_is_reachable(definition_source) {
            true => SourceMacroResolution::Resolved { definition },
            false => {
                SourceMacroResolution::Unavailable(SourcePreprocUnavailable::DetachedSource {
                    source: definition_source,
                })
            }
        }
    }

    /// Whether `source` is reachable from the root through include edges.
    fn source_is_reachable(&self, source: PreprocSourceId) -> bool {
        let mut current = source;
        loop {
            let Some(source) = self.model.sources.iter().find(|candidate| candidate.id == current)
            else {
                return false;
            };
            match source.origin {
                PreprocSourceOrigin::Root | PreprocSourceOrigin::Predefine => return true,
                PreprocSourceOrigin::Detached => return false,
                PreprocSourceOrigin::Included { include_event_id } => {
                    let directive = self
                        .model
                        .include_graph
                        .directives()
                        .iter()
                        .find(|directive| directive.event_id == include_event_id);
                    let Some(directive) = directive else {
                        return false;
                    };
                    current = directive.directive_range.source;
                }
            }
        }
    }

    pub(in crate::source::tables::builder) fn include_guard_definition_after_ifndef(
        &self,
        conditional_index: usize,
        name: &str,
    ) -> Option<SourceMacroDefinitionId> {
        let conditional = self.model.conditionals.get(conditional_index)?;
        if conditional.kind != MacroConditionalKind::IfNDef {
            return None;
        }

        let source = conditional.range.source;
        let (conditional_order, _) =
            self.model.event_records.iter().enumerate().find(|(_, directive)| {
                directive.kind == MacroEventKind::Conditional
                    && directive.index == conditional_index
            })?;
        for directive in self.model.event_records.iter().skip(conditional_order + 1) {
            if directive.range.source != source {
                continue;
            }
            match directive.kind {
                MacroEventKind::Define => {
                    let define = self.model.defines.get(directive.index)?;
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

    pub(in crate::source::tables::builder) fn record_missing_reference_name(
        &mut self,
        _event_id: SourcePreprocEventId,
    ) {
    }

    pub(in crate::source::tables::builder) fn record_missing_reference_name_range(
        &mut self,
        _event_id: SourcePreprocEventId,
    ) {
    }

    pub(in crate::source::tables::builder) fn record_state_checkpoint(
        &mut self,
        source_order: usize,
        _boundary: SourcePosition,
    ) {
        let id = SourceMacroStateId::new(self.model.state_timeline.states.len());
        self.model
            .state_timeline
            .states
            .push(SourceMacroState { id, definitions: self.current_state.clone() });
        self.model.state_timeline.checkpoints.push(SourceMacroStateCheckpoint {
            source_order,
            state: id,
        });
    }
}
