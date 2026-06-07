use super::*;

pub(in crate::preproc) fn require_file_backed_source(
    source: &MappedPreprocSource,
) -> PreprocResult<FileId> {
    source.file_id().ok_or_else(|| {
        let MappedPreprocSource::VirtualDisplay { path, origin } = source else {
            unreachable!("file-backed source should have a FileId");
        };
        PreprocError::SourceMap(PreprocSourceMapError::DisplayOnlyVirtualSource {
            path: path.clone(),
            origin: origin.clone(),
        })
    })
}

pub(in crate::preproc) fn map_source_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(FileId, TextRange)> {
    let (source, range) = map_mapped_source_range(mapped, source_range)?;
    Ok((require_file_backed_source(&source)?, range))
}

pub(in crate::preproc) fn map_source_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<FileId> {
    mapped.source_map.file_id(source).map_err(PreprocError::SourceMap)
}

pub(in crate::preproc) fn map_mapped_source_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(MappedPreprocSource, TextRange)> {
    let range = mapped.source_map.map_range(source_range).map_err(PreprocError::SourceMap)?;
    let source = map_mapped_source_id(mapped, source_range.source)?;
    Ok((source, range))
}

pub(in crate::preproc) fn mapped_source_range_at_offset(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<(MappedPreprocSource, TextRange)>> {
    let (source, range) = map_mapped_source_range(mapped, source_range)?;
    Ok((source.file_id() == Some(file_id) && range.contains(offset)).then_some((source, range)))
}

pub(in crate::preproc) fn map_mapped_source_id(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
) -> PreprocResult<MappedPreprocSource> {
    match mapped.source_map.get(source) {
        Some(PreprocSourceMapping::RealFile(file_id)) => {
            Ok(MappedPreprocSource::RealFile { file_id: *file_id })
        }
        Some(PreprocSourceMapping::VirtualFile { file_id, path, origin }) => {
            Ok(MappedPreprocSource::VirtualFile {
                file_id: *file_id,
                path: path.clone(),
                origin: origin.clone(),
            })
        }
        Some(PreprocSourceMapping::VirtualDisplay { path, origin }) => {
            Ok(MappedPreprocSource::VirtualDisplay { path: path.clone(), origin: origin.clone() })
        }
        Some(PreprocSourceMapping::Unmapped(reason)) => {
            Err(PreprocError::SourceMap(PreprocSourceMapError::UnmappedSource {
                source,
                reason: reason.clone(),
            }))
        }
        None => Err(PreprocError::SourceMap(PreprocSourceMapError::MissingSource { source })),
    }
}
