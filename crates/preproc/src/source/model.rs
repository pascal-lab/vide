use syntax::preproc::Trace;

use super::{tables::*, types::*};

impl SourcePreprocModel {
    pub fn new(index: SourcePreprocIndex) -> Self {
        SourcePreprocModelBuilder::new(index).build()
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

    pub fn state_timeline(&self) -> &SourceMacroStateTimeline {
        &self.state_timeline
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
            .filter_map(|definition_id| self.macro_definitions.get(*definition_id))
            .collect()
    }
}

#[cfg(test)]
mod tests;
