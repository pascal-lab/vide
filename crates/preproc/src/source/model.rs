use syntax::preproc::Trace;

use super::{provenance::*, types::*};

impl SourcePreprocModel {
    pub fn new(index: SourcePreprocIndex) -> Self {
        let tables = SourcePreprocTables::from_index(&index);
        Self { index, tables }
    }

    pub fn from_trace(trace: Trace) -> Result<Self, SourcePreprocError> {
        let index = SourcePreprocIndex::from_trace(trace)?;
        Ok(Self::new(index))
    }

    pub fn index(&self) -> &SourcePreprocIndex {
        &self.index
    }

    pub fn into_index(self) -> SourcePreprocIndex {
        self.index
    }

    pub fn provenance_tables(&self) -> &SourcePreprocTables {
        &self.tables
    }

    pub fn macro_definitions(&self) -> &SourceMacroDefinitionTable {
        &self.tables.macro_definitions
    }

    pub fn macro_references(&self) -> &SourceMacroReferenceTable {
        &self.tables.macro_references
    }

    pub fn macro_calls(&self) -> &SourceMacroCallTable {
        &self.tables.macro_calls
    }

    pub fn macro_expansions(&self) -> &SourceMacroExpansionTable {
        &self.tables.macro_expansions
    }

    pub fn emitted_tokens(&self) -> &SourceEmittedTokenTable {
        &self.tables.emitted_tokens
    }

    pub fn token_provenance(&self) -> &SourceTokenProvenanceTable {
        &self.tables.token_provenance
    }

    pub fn include_graph(&self) -> &SourceIncludeGraph {
        &self.tables.include_graph
    }

    pub fn state_timeline(&self) -> &SourceMacroStateTimeline {
        &self.tables.state_timeline
    }

    pub fn root_source(&self) -> Option<PreprocSourceId> {
        self.index.root_source
    }

    pub fn sources(&self) -> &[PreprocSource] {
        &self.index.sources
    }

    pub fn defines(&self) -> &[SourceMacroDefine] {
        &self.index.defines
    }

    pub fn undefs(&self) -> &[SourceMacroUndef] {
        &self.index.undefs
    }

    pub fn usages(&self) -> &[SourceMacroUsage] {
        &self.index.usages
    }

    pub fn includes(&self) -> &[SourceMacroInclude] {
        &self.index.includes
    }

    pub fn conditionals(&self) -> &[SourceMacroConditional] {
        &self.index.conditionals
    }

    pub fn inactive_ranges(&self) -> &[SourceRange] {
        &self.tables.inactive_ranges
    }

    pub fn events(&self) -> impl Iterator<Item = SourcePreprocEvent<'_>> + '_ {
        self.index
            .event_records
            .iter()
            .enumerate()
            .filter_map(|(source_order, directive)| self.event_from_record(source_order, directive))
    }

    pub fn visible_macros_at(&self, position: SourcePosition) -> Vec<&SourceMacroDefinition> {
        self.tables
            .state_timeline
            .state_at_position(position)
            .map(|state| self.definitions_for_state(state))
            .unwrap_or_default()
    }

    pub fn immediate_macro_expansion(&self, call: SourceMacroCallId) -> SourceMacroExpansionQuery {
        let Some(call_fact) = self.tables.macro_calls.get(call) else {
            return SourceMacroExpansionQuery::Unavailable(
                SourcePreprocUnavailable::MissingMacroCall { call },
            );
        };
        match (call_fact.expansion, &call_fact.status) {
            (Some(expansion), SourceMacroCallStatus::ExpansionAvailable)
                if self.tables.macro_expansions.get(expansion).is_some() =>
            {
                SourceMacroExpansionQuery::Available(expansion)
            }
            (Some(expansion), SourceMacroCallStatus::ExpansionAvailable) => {
                SourceMacroExpansionQuery::Unavailable(
                    SourcePreprocUnavailable::MissingMacroExpansion {
                        call: self
                            .tables
                            .macro_expansions
                            .get(expansion)
                            .map(|expansion| expansion.call)
                            .unwrap_or(call),
                    },
                )
            }
            (_, SourceMacroCallStatus::ExpansionUnavailable(reason)) => {
                SourceMacroExpansionQuery::Unavailable(reason.clone())
            }
            (None, SourceMacroCallStatus::ExpansionAvailable) => {
                SourceMacroExpansionQuery::Unavailable(
                    SourcePreprocUnavailable::MissingMacroExpansion { call },
                )
            }
        }
    }

    pub fn recursive_macro_expansion(
        &self,
        call: SourceMacroCallId,
    ) -> SourceRecursiveMacroExpansion {
        let mut result = SourceRecursiveMacroExpansion {
            root_call: call,
            expansions: Vec::new(),
            unavailable: Vec::new(),
        };
        self.collect_recursive_macro_expansion(call, &mut result, &mut Vec::new());
        result
    }

    pub fn provenance(&self, entity: SourcePreprocEntity) -> Option<SourcePreprocProvenance> {
        let (event_id, name, range, name_range) = match entity {
            SourcePreprocEntity::Define(index) => {
                let define = self.index.defines.get(index)?;
                (define.event_id, define.name.clone(), define.range, define.name_range)
            }
            SourcePreprocEntity::Undef(index) => {
                let undef = self.index.undefs.get(index)?;
                (undef.event_id, undef.name.clone(), undef.range, undef.name_range)
            }
            SourcePreprocEntity::Usage(index) => {
                let usage = self.index.usages.get(index)?;
                (usage.event_id, usage.name.clone(), usage.range, usage.name_range)
            }
            SourcePreprocEntity::Include(index) => {
                let include = self.index.includes.get(index)?;
                (include.event_id, None, include.range, include.target_range)
            }
            SourcePreprocEntity::Conditional(index) => {
                let conditional = self.index.conditionals.get(index)?;
                (conditional.event_id, None, conditional.range, None)
            }
        };
        Some(SourcePreprocProvenance { event_id, entity, name, range, name_range })
    }

    pub fn source_range(&self, entity: SourcePreprocEntity) -> Option<SourceRange> {
        self.provenance(entity).map(|provenance| provenance.range)
    }

    pub fn define(&self, index: usize) -> Option<&SourceMacroDefine> {
        self.index.defines.get(index)
    }

    pub fn undef(&self, index: usize) -> Option<&SourceMacroUndef> {
        self.index.undefs.get(index)
    }

    pub fn usage(&self, index: usize) -> Option<&SourceMacroUsage> {
        self.index.usages.get(index)
    }

    pub fn include(&self, index: usize) -> Option<&SourceMacroInclude> {
        self.index.includes.get(index)
    }

    pub fn conditional(&self, index: usize) -> Option<&SourceMacroConditional> {
        self.index.conditionals.get(index)
    }

    fn definitions_for_state(&self, state: &SourceMacroState) -> Vec<&SourceMacroDefinition> {
        state
            .definitions
            .values()
            .filter_map(|definition_id| self.tables.macro_definitions.get(*definition_id))
            .collect()
    }

    fn collect_recursive_macro_expansion(
        &self,
        call: SourceMacroCallId,
        result: &mut SourceRecursiveMacroExpansion,
        visiting: &mut Vec<SourceMacroCallId>,
    ) {
        if visiting.contains(&call) {
            result.unavailable.push(SourceMacroExpansionUnavailable {
                call,
                reason: SourcePreprocUnavailable::MissingMacroExpansion { call },
            });
            return;
        }

        match self.immediate_macro_expansion(call) {
            SourceMacroExpansionQuery::Available(expansion_id) => {
                if result.expansions.contains(&expansion_id) {
                    return;
                }
                result.expansions.push(expansion_id);
                let Some(expansion) = self.tables.macro_expansions.get(expansion_id) else {
                    result.unavailable.push(SourceMacroExpansionUnavailable {
                        call,
                        reason: SourcePreprocUnavailable::MissingMacroExpansion { call },
                    });
                    return;
                };
                visiting.push(call);
                for child in &expansion.child_calls {
                    self.collect_recursive_macro_expansion(*child, result, visiting);
                }
                visiting.pop();
            }
            SourceMacroExpansionQuery::Unavailable(reason) => {
                result.unavailable.push(SourceMacroExpansionUnavailable { call, reason });
            }
        }
    }
}

mod events;

#[cfg(test)]
mod tests;
