use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MacroDefinitionKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroDefinitionKey {
    pub(crate) fn from_definition(definition: &MacroDefinition) -> Self {
        Self {
            file_id: definition.file_id,
            range_start: definition.name_range.start(),
            range_end: definition.name_range.end(),
            name: definition.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct MacroReferenceKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroReferenceKey {
    pub(crate) fn from_reference(reference: &MacroReference) -> Self {
        Self {
            file_id: reference.file_id,
            range_start: reference.range.start(),
            range_end: reference.range.end(),
            name: reference.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MacroParamDefinitionKey {
    macro_definition: MacroDefinitionKey,
    param_index: usize,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroParamDefinitionKey {
    pub(crate) fn from_definition(definition: &MacroParamDefinition) -> Self {
        Self {
            macro_definition: MacroDefinitionKey::from_definition(&definition.macro_definition),
            param_index: definition.param_index,
            range_start: definition.range.start(),
            range_end: definition.range.end(),
            name: definition.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct MacroParamReferenceKey {
    macro_definition: MacroDefinitionKey,
    param_index: usize,
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
    name: SmolStr,
}

impl MacroParamReferenceKey {
    pub(crate) fn from_reference(reference: &MacroParamReference) -> Self {
        Self {
            macro_definition: MacroDefinitionKey::from_definition(&reference.macro_definition),
            param_index: reference.param_index,
            file_id: reference.file_id,
            range_start: reference.range.start(),
            range_end: reference.range.end(),
            name: reference.name.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InactiveBranchKey {
    file_id: FileId,
    range_start: TextSize,
    range_end: TextSize,
}

impl InactiveBranchKey {
    pub(crate) fn from_branch(branch: &InactiveBranch) -> Self {
        Self {
            file_id: branch.file_id,
            range_start: branch.range.start(),
            range_end: branch.range.end(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroReferenceIndex {
    references_by_definition:
        BTreeMap<MacroDefinitionKey, UniqVec<MacroReference, MacroReferenceKey>>,
    definitions_by_reference:
        BTreeMap<MacroReferenceKey, UniqVec<MacroDefinition, MacroDefinitionKey>>,
    issues: Vec<MacroReferenceIndexIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferences {
    pub references: Vec<MacroReference>,
    pub status: MacroReferenceIndexStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroReferenceIndexStatus {
    Complete,
    Partial { issue_count: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MacroReferenceIndexIssue {
    SkippedModel { file_id: FileId, error: PreprocError },
    UnavailableReference { file_id: FileId, reason: SourcePreprocUnavailable },
}

impl MacroReferenceIndex {
    pub fn references_for(&self, definition: &MacroDefinition) -> Vec<MacroReference> {
        self.references_by_definition
            .get(&MacroDefinitionKey::from_definition(definition))
            .map(|references| references.as_slice().to_vec())
            .unwrap_or_default()
    }

    pub fn definitions_for_reference(
        &self,
        reference: &MacroReference,
    ) -> Option<&[MacroDefinition]> {
        self.definitions_by_reference
            .get(&MacroReferenceKey::from_reference(reference))
            .map(UniqVec::as_slice)
    }

    pub fn status(&self) -> MacroReferenceIndexStatus {
        if self.issues.is_empty() {
            MacroReferenceIndexStatus::Complete
        } else {
            MacroReferenceIndexStatus::Partial { issue_count: self.issues.len() }
        }
    }

    pub(in crate::preproc) fn push(
        &mut self,
        definition: MacroDefinition,
        reference: MacroReference,
    ) {
        let definition_key = MacroDefinitionKey::from_definition(&definition);
        let references = self.references_by_definition.entry(definition_key).or_default();
        references.push([MacroReferenceKey::from_reference(&reference)], reference.clone());

        let reference_key = MacroReferenceKey::from_reference(&reference);
        let definitions = self.definitions_by_reference.entry(reference_key).or_default();
        definitions.push([MacroDefinitionKey::from_definition(&definition)], definition);
    }

    pub(in crate::preproc) fn push_issue(&mut self, issue: MacroReferenceIndexIssue) {
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }
}
