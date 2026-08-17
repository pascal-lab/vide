use syntax::preproc::Trace;

use super::{tables::*, types::*};

impl SourcePreprocModel {
    /// Build the model in a single pass from the slang preprocessor trace.
    pub fn from_trace(trace: &Trace) -> Result<Self, SourcePreprocError> {
        Ok(SourcePreprocModelBuilder::collect(trace)?.build())
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
