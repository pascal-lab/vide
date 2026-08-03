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
    PreprocError::SourceModel(reason)
}
