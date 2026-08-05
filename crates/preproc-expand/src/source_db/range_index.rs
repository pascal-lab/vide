use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedSourcePreprocModel {
    pub model: SourcePreprocModel,
    pub source_map: PreprocSourceMap,
    range_index: PreprocRangeIndex,
}

impl MappedSourcePreprocModel {
    pub(super) fn new(model: SourcePreprocModel, source_map: PreprocSourceMap) -> Self {
        let range_index = PreprocRangeIndex::from_model(&model, &source_map);
        Self { model, source_map, range_index }
    }

    pub fn macro_reference_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<SourceMacroReferenceId> {
        self.range_index.reference_ids_at(file_id, offset)
    }

    pub fn macro_reference_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroReferenceId> {
        self.range_index.reference_ids_intersecting_range(file_id, range)
    }

    pub fn macro_call_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroCallId> {
        self.range_index.call_ids_at(file_id, offset)
    }

    pub fn macro_call_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroCallId> {
        self.range_index.call_ids_intersecting_range(file_id, range)
    }

    pub fn macro_definition_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<SourceMacroDefinitionId> {
        self.range_index.definition_ids_at(file_id, offset)
    }

    pub fn macro_param_definition_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<(SourceMacroDefinitionId, usize)> {
        self.range_index.param_definition_ids_at(file_id, offset)
    }

    pub fn macro_param_reference_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<(SourceMacroDefinitionId, usize)> {
        self.range_index.param_reference_ids_at(file_id, offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PreprocRangeIndex {
    references_by_file: FxHashMap<FileId, Vec<IndexedRange<SourceMacroReferenceId>>>,
    calls_by_file: FxHashMap<FileId, Vec<IndexedRange<SourceMacroCallId>>>,
    definitions_by_file: FxHashMap<FileId, Vec<IndexedRange<SourceMacroDefinitionId>>>,
    /// Param name tokens, keyed by (definition, param index).
    param_definitions_by_file:
        FxHashMap<FileId, Vec<IndexedRange<(SourceMacroDefinitionId, usize)>>>,
    /// Param use tokens inside definition bodies, keyed by (definition, token
    /// index).
    param_references_by_file:
        FxHashMap<FileId, Vec<IndexedRange<(SourceMacroDefinitionId, usize)>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexedRange<T> {
    range: TextRange,
    id: T,
}

impl PreprocRangeIndex {
    fn from_model(model: &SourcePreprocModel, source_map: &PreprocSourceMap) -> Self {
        let mut index = Self::default();
        for reference in model.macro_references().iter() {
            if let Some((file_id, range)) = mapped_file_range(source_map, reference.name_range) {
                index
                    .references_by_file
                    .entry(file_id)
                    .or_default()
                    .push(IndexedRange { range, id: reference.id });
            }
        }
        for call in model.macro_calls().iter() {
            if let Some((file_id, range)) = mapped_file_range(source_map, call.call_range) {
                index
                    .calls_by_file
                    .entry(file_id)
                    .or_default()
                    .push(IndexedRange { range, id: call.id });
            }
        }
        for definition in model.macro_definitions().iter() {
            if let Some((file_id, range)) = definition_file_range(source_map, definition.name_range)
            {
                index
                    .definitions_by_file
                    .entry(file_id)
                    .or_default()
                    .push(IndexedRange { range, id: definition.id });
            }
            let Some(params) = &definition.params else {
                continue;
            };
            for (param_index, param) in params.iter().enumerate() {
                let Some(name_range) = param.name_range else {
                    continue;
                };
                if let Some((file_id, range)) = mapped_file_range(source_map, name_range) {
                    index
                        .param_definitions_by_file
                        .entry(file_id)
                        .or_default()
                        .push(IndexedRange { range, id: (definition.id, param_index) });
                }
            }
            for (token_index, token) in definition.body_tokens.iter().enumerate() {
                let Some(token_range) = token.range else {
                    continue;
                };
                let is_param_use =
                    params.iter().any(|param| param.name.as_ref() == Some(&token.value));
                if !is_param_use {
                    continue;
                }
                if let Some((file_id, range)) = mapped_file_range(source_map, token_range) {
                    index
                        .param_references_by_file
                        .entry(file_id)
                        .or_default()
                        .push(IndexedRange { range, id: (definition.id, token_index) });
                }
            }
        }
        for references in index.references_by_file.values_mut() {
            sort_indexed_ranges(references);
        }
        for calls in index.calls_by_file.values_mut() {
            sort_indexed_ranges(calls);
        }
        for definitions in index.definitions_by_file.values_mut() {
            sort_indexed_ranges(definitions);
        }
        for definitions in index.param_definitions_by_file.values_mut() {
            sort_indexed_ranges(definitions);
        }
        for references in index.param_references_by_file.values_mut() {
            sort_indexed_ranges(references);
        }
        index
    }

    fn reference_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroReferenceId> {
        ids_at(self.references_by_file.get(&file_id), offset)
    }

    fn reference_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroReferenceId> {
        ids_intersecting_range(self.references_by_file.get(&file_id), range)
    }

    fn call_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroCallId> {
        ids_at(self.calls_by_file.get(&file_id), offset)
    }

    fn call_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroCallId> {
        ids_intersecting_range(self.calls_by_file.get(&file_id), range)
    }

    fn definition_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroDefinitionId> {
        ids_at(self.definitions_by_file.get(&file_id), offset)
    }

    fn param_definition_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<(SourceMacroDefinitionId, usize)> {
        ids_at(self.param_definitions_by_file.get(&file_id), offset)
    }

    fn param_reference_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<(SourceMacroDefinitionId, usize)> {
        ids_at(self.param_references_by_file.get(&file_id), offset)
    }
}

fn mapped_file_range(
    source_map: &PreprocSourceMap,
    source_range: SourceRange,
) -> Option<(FileId, TextRange)> {
    let range = source_map.map_range(source_range).ok()?;
    let file_id = source_map.file_id(source_range.source).ok()?;
    Some((file_id, range))
}

/// Maps a macro definition's name range to its backing file, applying the
/// manifest-predefine remap so indexed ranges match what
/// `map_macro_definition` reports.
fn definition_file_range(
    source_map: &PreprocSourceMap,
    name_range: SourceRange,
) -> Option<(FileId, TextRange)> {
    if let Some(manifest) = source_map.predefine_manifest_source(name_range.source) {
        return Some((manifest.file_id, manifest.range));
    }
    mapped_file_range(source_map, name_range)
}

fn sort_indexed_ranges<T: Copy>(ranges: &mut [IndexedRange<T>]) {
    ranges.sort_by_key(|entry| (entry.range.start(), entry.range.end()));
}

fn ids_at<T: Copy>(ranges: Option<&Vec<IndexedRange<T>>>, offset: TextSize) -> Vec<T> {
    let Some(ranges) = ranges else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for entry in ranges {
        if entry.range.start() > offset {
            break;
        }
        if entry.range.contains(offset) {
            ids.push(entry.id);
        }
    }
    ids
}

fn ids_intersecting_range<T: Copy>(
    ranges: Option<&Vec<IndexedRange<T>>>,
    range: TextRange,
) -> Vec<T> {
    let Some(ranges) = ranges else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for entry in ranges {
        if entry.range.start() >= range.end() {
            break;
        }
        if entry.range.intersect(range).is_some_and(|intersection| !intersection.is_empty()) {
            ids.push(entry.id);
        }
    }
    ids
}
