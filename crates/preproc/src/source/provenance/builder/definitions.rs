use super::*;

impl<'a> SourcePreprocModelBuilder<'a> {
    pub(in crate::source::provenance::builder) fn build_definition_table(&mut self) {
        for (define_index, define) in self.index.defines.iter().enumerate() {
            let Some(name) = define.name.clone() else {
                self.definition_ranges_partial = true;
                self.tables.issues.push(SourcePreprocFactIssue::MissingDefinitionName {
                    event_id: define.event_id,
                });
                continue;
            };
            let Some(name_range) = define.name_range else {
                self.definition_ranges_partial = true;
                self.tables.issues.push(SourcePreprocFactIssue::MissingDefinitionNameRange {
                    event_id: define.event_id,
                });
                continue;
            };

            let id = SourceMacroDefinitionId::new(self.tables.macro_definitions.len());
            self.tables.macro_definitions.push(SourceMacroDefinition {
                id,
                event_id: define.event_id,
                identity: define.identity,
                name,
                name_range,
                directive_range: define.range,
                params: define.params.clone(),
                body_tokens: define.body.clone(),
            });
            self.definition_ids_by_define_index.insert(define_index, id);
            if let Some(identity) = define.identity {
                self.definition_ids_by_identity.insert(identity, id);
            }
        }
    }
}
