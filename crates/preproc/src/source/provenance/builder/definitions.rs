use super::*;

impl SourcePreprocModelBuilder {
    pub(in crate::source::provenance::builder) fn build_definition_table(&mut self) {
        for (define_index, define) in self.model.index.defines.iter().enumerate() {
            let Some(name) = define.name.clone() else {
                self.definition_ranges_partial = true;
                self.model
                    .issues
                    .push(SourcePreprocIssue::MissingDefinitionName { event_id: define.event_id });
                continue;
            };
            let Some(name_range) = define.name_range else {
                self.definition_ranges_partial = true;
                self.model.issues.push(SourcePreprocIssue::MissingDefinitionNameRange {
                    event_id: define.event_id,
                });
                continue;
            };

            let id = SourceMacroDefinitionId::new(self.model.macro_definitions.len());
            self.model.macro_definitions.push(SourceMacroDefinition {
                id,
                event_id: define.event_id,
                trace_definition: define.trace_definition,
                name,
                name_range,
                directive_range: define.range,
                params: define.params.clone(),
                body_tokens: define.body.clone(),
            });
            self.definition_ids_by_define_index.insert(define_index, id);
            if let Some(trace_definition) = define.trace_definition {
                self.definitions_by_trace_id.insert(trace_definition, id);
            }
        }
    }
}
