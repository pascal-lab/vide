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
    references_by_file: FxHashMap<FileId, RangeIndex<SourceMacroReferenceId>>,
    calls_by_file: FxHashMap<FileId, RangeIndex<SourceMacroCallId>>,
    definitions_by_file: FxHashMap<FileId, RangeIndex<SourceMacroDefinitionId>>,
    /// Param name tokens, keyed by (definition, param index).
    param_definitions_by_file: FxHashMap<FileId, RangeIndex<(SourceMacroDefinitionId, usize)>>,
    /// Param use tokens inside definition bodies, keyed by (definition, token
    /// index).
    param_references_by_file: FxHashMap<FileId, RangeIndex<(SourceMacroDefinitionId, usize)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IndexedRange<T> {
    range: TextRange,
    id: T,
}

/// Ranges for one file kept in two sort views. Range end-points are not
/// monotonic (ranges overlap and nest), so an exact interval-stabbing query
/// scans the smaller of the two candidate sets: ranges whose start precedes
/// the probe point versus ranges whose end follows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RangeIndex<T> {
    by_start: Vec<IndexedRange<T>>,
    by_end: Vec<IndexedRange<T>>,
}

impl<T> Default for RangeIndex<T> {
    fn default() -> Self {
        Self { by_start: Vec::new(), by_end: Vec::new() }
    }
}

impl<T: Copy> RangeIndex<T> {
    pub(crate) fn push(&mut self, range: TextRange, id: T) {
        self.by_start.push(IndexedRange { range, id });
        self.by_end.push(IndexedRange { range, id });
    }

    pub(crate) fn finish(&mut self) {
        self.by_start.sort_by_key(|entry| (entry.range.start(), entry.range.end()));
        self.by_end.sort_by_key(|entry| (entry.range.end(), entry.range.start()));
    }

    /// All ids whose range contains `offset` (half-open: `start <= offset <
    /// end`), in start order.
    pub(crate) fn ids_at(&self, offset: TextSize) -> Vec<T> {
        let start_prefix = self.by_start.partition_point(|entry| entry.range.start() <= offset);
        let end_suffix = self.by_end.partition_point(|entry| entry.range.end() <= offset);
        let mut hits = Vec::new();
        if start_prefix <= self.by_end.len() - end_suffix {
            hits.extend_from_slice(&self.by_start[..start_prefix]);
            hits.retain(|entry| entry.range.end() > offset);
        } else {
            hits.extend_from_slice(&self.by_end[end_suffix..]);
            hits.retain(|entry| entry.range.start() <= offset);
            // The end-sorted view is not in start order; restore the stable
            // start order callers observe (e.g. nested macro call targets).
            hits.sort_unstable_by_key(|entry| entry.range.start());
        }
        hits.into_iter().map(|entry| entry.id).collect()
    }

    /// All ids whose range intersects `range` (non-empty intersection), in
    /// start order.
    pub(crate) fn ids_intersecting_range(&self, range: TextRange) -> Vec<T> {
        let start_prefix = self.by_start.partition_point(|entry| entry.range.start() < range.end());
        let end_suffix = self.by_end.partition_point(|entry| entry.range.end() <= range.start());
        let mut hits = Vec::new();
        if start_prefix <= self.by_end.len() - end_suffix {
            hits.extend_from_slice(&self.by_start[..start_prefix]);
            hits.retain(|entry| entry.range.end() > range.start());
        } else {
            hits.extend_from_slice(&self.by_end[end_suffix..]);
            hits.retain(|entry| entry.range.start() < range.end());
            hits.sort_unstable_by_key(|entry| entry.range.start());
        }
        hits.into_iter().map(|entry| entry.id).collect()
    }
}

impl PreprocRangeIndex {
    fn from_model(model: &SourcePreprocModel, source_map: &PreprocSourceMap) -> Self {
        let mut index = Self::default();
        for reference in model.macro_references().iter() {
            if let Some((file_id, range)) = mapped_file_range(source_map, reference.name_range) {
                index.references_by_file.entry(file_id).or_default().push(range, reference.id);
            }
        }
        for call in model.macro_calls().iter() {
            if let Some((file_id, range)) = mapped_file_range(source_map, call.call_range) {
                index.calls_by_file.entry(file_id).or_default().push(range, call.id);
            }
        }
        for definition in model.macro_definitions().iter() {
            if let Some((file_id, range)) = definition_file_range(source_map, definition.name_range)
            {
                index.definitions_by_file.entry(file_id).or_default().push(range, definition.id);
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
                        .push(range, (definition.id, param_index));
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
                        .push(range, (definition.id, token_index));
                }
            }
        }
        for entries in index.references_by_file.values_mut() {
            entries.finish();
        }
        for calls in index.calls_by_file.values_mut() {
            calls.finish();
        }
        for definitions in index.definitions_by_file.values_mut() {
            definitions.finish();
        }
        for definitions in index.param_definitions_by_file.values_mut() {
            definitions.finish();
        }
        for references in index.param_references_by_file.values_mut() {
            references.finish();
        }
        index
    }

    fn reference_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroReferenceId> {
        ids_at(&self.references_by_file, file_id, offset)
    }

    fn reference_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroReferenceId> {
        ids_intersecting_range(&self.references_by_file, file_id, range)
    }

    fn call_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroCallId> {
        ids_at(&self.calls_by_file, file_id, offset)
    }

    fn call_ids_intersecting_range(
        &self,
        file_id: FileId,
        range: TextRange,
    ) -> Vec<SourceMacroCallId> {
        ids_intersecting_range(&self.calls_by_file, file_id, range)
    }

    fn definition_ids_at(&self, file_id: FileId, offset: TextSize) -> Vec<SourceMacroDefinitionId> {
        ids_at(&self.definitions_by_file, file_id, offset)
    }

    fn param_definition_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<(SourceMacroDefinitionId, usize)> {
        ids_at(&self.param_definitions_by_file, file_id, offset)
    }

    fn param_reference_ids_at(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Vec<(SourceMacroDefinitionId, usize)> {
        ids_at(&self.param_references_by_file, file_id, offset)
    }
}

fn mapped_file_range(
    source_map: &PreprocSourceMap,
    source_range: SourceRange,
) -> Option<(FileId, TextRange)> {
    let range = match source_map.map_range(source_range) {
        Ok(range) => range,
        Err(SourcePreprocQueryError::DisplayOnlyVirtualSource { .. }) => return None,
        Err(error) => {
            tracing::warn!(?source_range, ?error, "dropping unmapped preprocessor index range");
            return None;
        }
    };
    let file_id = match source_map.file_id(source_range.source) {
        Ok(file_id) => file_id,
        Err(SourcePreprocQueryError::DisplayOnlyVirtualSource { .. }) => return None,
        Err(error) => {
            tracing::warn!(?source_range, ?error, "dropping preprocessor index range without a file");
            return None;
        }
    };
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

fn ids_at<T: Copy>(
    by_file: &FxHashMap<FileId, RangeIndex<T>>,
    file_id: FileId,
    offset: TextSize,
) -> Vec<T> {
    by_file.get(&file_id).map_or_else(Vec::new, |index| index.ids_at(offset))
}

fn ids_intersecting_range<T: Copy>(
    by_file: &FxHashMap<FileId, RangeIndex<T>>,
    file_id: FileId,
    range: TextRange,
) -> Vec<T> {
    by_file.get(&file_id).map_or_else(Vec::new, |index| index.ids_intersecting_range(range))
}
