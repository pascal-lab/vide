use super::*;

pub(in crate::preproc) fn map_source_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(FileId, TextRange)> {
    mapped.source_map.map_range(source_range).map_err(PreprocError::SourceMap)
}

pub(in crate::preproc) fn map_source_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<FileId> {
    mapped.source_map.file_id(source).map_err(PreprocError::SourceMap)
}

pub(in crate::preproc) fn source_mapping_range_at_offset(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<(FileId, TextRange)>> {
    let (source_file_id, range) = map_source_range(mapped, source_range)?;
    Ok((source_file_id == file_id && range.contains(offset)).then_some((source_file_id, range)))
}
