use super::*;

pub(in crate::preproc) fn map_include_status(
    mapped: &MappedSourcePreprocModel,
    status: &SourceIncludeStatus,
) -> PreprocResult<IncludeDirectiveStatus> {
    Ok(match status {
        SourceIncludeStatus::Resolved { source } => {
            IncludeDirectiveStatus::Resolved { source: map_mapped_source_id(mapped, *source)? }
        }
        SourceIncludeStatus::Unresolved => IncludeDirectiveStatus::Unresolved,
        SourceIncludeStatus::Unavailable(reason) => {
            IncludeDirectiveStatus::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    })
}

pub(in crate::preproc) fn unavailable_error(reason: SourcePreprocUnavailable) -> PreprocError {
    PreprocError::Unavailable { reason: PreprocUnavailable::Source(reason) }
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
            Ok(IncludeChainEntry {
                include_event_id: entry.include_event_id.raw(),
                include_file_id,
                include_range,
                included_file_id,
            })
        })
        .collect()
}
