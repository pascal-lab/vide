use super::*;

pub(in crate::preproc) fn map_include_resolved_file(
    mapped: &MappedSourcePreprocModel,
    status: &SourceIncludeStatus,
) -> PreprocResult<Option<FileId>> {
    match status {
        SourceIncludeStatus::Resolved { source } => Ok(Some(map_source_id(mapped, *source)?)),
        SourceIncludeStatus::Unresolved | SourceIncludeStatus::Unavailable(_) => Ok(None),
    }
}

pub(in crate::preproc) fn source_model_error(reason: SourcePreprocUnavailable) -> PreprocError {
    PreprocError::SourceModel(reason.into())
}

pub(in crate::preproc) fn map_include_chain(
    mapped: &MappedSourcePreprocModel,
    chain: &[SourceIncludeChainEntry],
) -> PreprocResult<Vec<IncludeChainEntry>> {
    chain
        .iter()
        .map(|entry| {
            let (include_file_id, include_range) = map_source_range(mapped, entry.include_range)?;
            let included_file_id = map_source_id(mapped, entry.included_source)?;
            Ok(IncludeChainEntry { include_file_id, include_range, included_file_id })
        })
        .collect()
}
