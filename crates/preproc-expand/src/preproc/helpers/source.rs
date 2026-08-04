use super::*;

pub(in crate::preproc) fn require_file_backed_source(
    source: &PreprocSourceMapping,
) -> PreprocResult<FileId> {
    match source {
        PreprocSourceMapping::RealFile(file_id)
        | PreprocSourceMapping::VirtualFile { file_id: Some(file_id), .. } => Ok(*file_id),
        PreprocSourceMapping::VirtualFile { path, origin, .. } => {
            Err(PreprocError::SourceQuery(SourcePreprocQueryError::DisplayOnlyVirtualSource {
                path: path.clone(),
                origin: origin.clone(),
            }))
        }
        PreprocSourceMapping::Unmapped(reason) => Err(PreprocError::SourceQuery(
            SourcePreprocQueryError::SourceUnavailable(reason.clone()),
        )),
    }
}

pub(in crate::preproc) fn map_source_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<FileId> {
    mapped.source_map.file_id(source).map_err(PreprocError::SourceQuery)
}

pub(in crate::preproc) fn map_source_mapping_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(PreprocSourceMapping, TextRange)> {
    let range = mapped.source_map.map_range(source_range).map_err(PreprocError::SourceQuery)?;
    let source = map_source_mapping_id(mapped, source_range.source)?;
    Ok((source, range))
}

pub(in crate::preproc) fn source_mapping_range_at_offset(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<(FileId, TextRange)>> {
    let (source, range) = map_source_mapping_range(mapped, source_range)?;
    let source_file_id = require_file_backed_source(&source)?;
    Ok((source_file_id == file_id && range.contains(offset)).then_some((source_file_id, range)))
}

pub(in crate::preproc) fn map_source_mapping_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<PreprocSourceMapping> {
    match mapped.source_map.get(source) {
        Some(mapping) => Ok(mapping.clone()),
        None => Err(PreprocError::SourceQuery(SourcePreprocQueryError::MissingSource { source })),
    }
}
