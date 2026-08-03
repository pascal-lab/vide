use syntax::preproc::Trace;

use super::{tables::*, trace::collect_model, types::*};

impl SourcePreprocModel {
    /// Build the model in a single pass from the slang preprocessor trace.
    pub fn from_trace(trace: Trace) -> Result<Self, SourcePreprocError> {
        let model = collect_model(trace)?;
        Ok(SourcePreprocModelBuilder::new(model).build())
    }

    pub fn macro_definitions(&self) -> &SourceMacroDefinitionTable {
        &self.macro_definitions
    }

    pub fn macro_references(&self) -> &SourceMacroReferenceTable {
        &self.macro_references
    }

    pub fn macro_calls(&self) -> &SourceMacroCallTable {
        &self.macro_calls
    }

    pub fn macro_expansions(&self) -> &SourceMacroExpansionTable {
        &self.macro_expansions
    }

    pub fn emitted_tokens(&self) -> &SourceEmittedTokenTable {
        &self.emitted_tokens
    }

    pub fn token_origins(&self) -> &SourceTokenOriginTable {
        &self.token_origins
    }

    pub fn include_graph(&self) -> &SourceIncludeGraph {
        &self.include_graph
    }

    pub fn inactive_ranges(&self) -> &[SourceRange] {
        &self.inactive_ranges
    }

    pub fn visible_macros_at(&self, position: SourcePosition) -> Vec<&SourceMacroDefinition> {
        self.state_timeline
            .state_at_position(position)
            .map(|state| self.definitions_for_state(state))
            .unwrap_or_default()
    }

    pub fn immediate_macro_expansion(
        &self,
        call: SourceMacroCallId,
    ) -> Result<SourceMacroExpansionId, SourcePreprocUnavailable> {
        let Some(call_fact) = self.macro_calls.get(call) else {
            return Err(SourcePreprocUnavailable::MissingMacroCall { call });
        };
        match &call_fact.expansion {
            Ok(expansion) if self.macro_expansions.get(*expansion).is_some() => Ok(*expansion),
            Ok(expansion) => Err(SourcePreprocUnavailable::MissingMacroExpansion {
                call: self
                    .macro_expansions
                    .get(*expansion)
                    .map(|expansion| expansion.call)
                    .unwrap_or(call),
            }),
            Err(reason) => Err(reason.clone()),
        }
    }

    fn definitions_for_state(&self, state: &SourceMacroState) -> Vec<&SourceMacroDefinition> {
        state
            .definitions
            .values()
            .filter_map(|definition_id| self.macro_definitions.get(*definition_id))
            .collect()
    }
}

#[cfg(test)]
mod tests;
