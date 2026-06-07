use super::*;

pub(in crate::preproc) fn mapped_result(
    result: &Result<MappedSourcePreprocModel, SourcePreprocQueryError>,
) -> PreprocResult<&MappedSourcePreprocModel> {
    result.as_ref().map_err(|err| err.clone().into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preproc) struct SourcePreprocQueryContexts {
    pub(in crate::preproc) model_file_ids: Vec<FileId>,
    pub(in crate::preproc) status: SourcePreprocContextStatus,
}

impl SourcePreprocQueryContexts {
    fn partial_error(&self) -> Option<PreprocError> {
        let SourcePreprocContextStatus::Partial { skipped_models } = self.status else {
            return None;
        };
        Some(PreprocError::Unavailable {
            reason: PreprocUnavailable::PartialPreprocContextIndex { skipped_models },
        })
    }
}

pub(in crate::preproc) fn source_preproc_single_query_contexts(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> SourcePreprocQueryContexts {
    let profile_id = db.file_compilation_profile(file_id);
    let index = db.source_preproc_context_index_for_profile(profile_id);
    let relevant = index.relevant_contexts(file_id);
    let mut file_ids = UniqVec::<FileId, FileId>::default();
    if matches!(
        db.file_kind(file_id),
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
    ) {
        file_ids.push_unique(file_id);
    }
    for model_file_id in relevant.model_file_ids {
        file_ids.push_unique(model_file_id);
    }
    SourcePreprocQueryContexts { model_file_ids: file_ids.into_vec(), status: relevant.status }
}

pub(in crate::preproc) fn finish_empty_single_query(
    contexts: &SourcePreprocQueryContexts,
    first_error: Option<PreprocError>,
) -> PreprocResult<()> {
    if let Some(error) = first_error {
        return Err(error);
    }
    if let Some(error) = contexts.partial_error() {
        return Err(error);
    }
    Ok(())
}

pub(in crate::preproc) fn record_first_error(
    first_error: &mut Option<PreprocError>,
    error: PreprocError,
) {
    if first_error.is_none() {
        *first_error = Some(error);
    }
}

pub(in crate::preproc) trait PreprocSingleExt<T> {
    fn into_single_or_none<F>(self, ambiguous: F) -> PreprocResult<Option<T>>
    where
        F: FnOnce(usize) -> PreprocUnavailable;

    fn into_exactly_one<F>(self, ambiguous: F) -> PreprocResult<T>
    where
        F: FnOnce(usize) -> PreprocUnavailable;
}

impl<T> PreprocSingleExt<T> for Vec<T> {
    fn into_single_or_none<F>(mut self, ambiguous: F) -> PreprocResult<Option<T>>
    where
        F: FnOnce(usize) -> PreprocUnavailable,
    {
        match self.len() {
            0 => Ok(None),
            1 => Ok(self.pop()),
            contexts => Err(PreprocError::Unavailable { reason: ambiguous(contexts) }),
        }
    }

    fn into_exactly_one<F>(mut self, ambiguous: F) -> PreprocResult<T>
    where
        F: FnOnce(usize) -> PreprocUnavailable,
    {
        match self.len() {
            1 => Ok(self.pop().unwrap()),
            contexts => Err(PreprocError::Unavailable { reason: ambiguous(contexts) }),
        }
    }
}
