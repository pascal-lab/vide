use super::*;

pub(in crate::preproc) fn map_expansion_source_buffer(
    mapped: &MappedSourcePreprocModel,
    expansion: SourceMacroExpansionId,
) -> PreprocResult<PreprocSourceMapping> {
    mapped.source_map.expansion_source_buffer(expansion).map_err(PreprocError::SourceMap)
}

pub(in crate::preproc) fn source_macro_calls_intersecting_range(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    source_range: TextRange,
) -> Vec<&SourceMacroCall> {
    mapped
        .macro_call_ids_intersecting_range(file_id, source_range)
        .into_iter()
        .filter_map(|call| mapped.model.macro_calls().get(call))
        .collect()
}
