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
        Some(PreprocError::PartialPreprocContextIndex { skipped_models })
    }
}

pub(in crate::preproc) fn source_preproc_single_query_contexts(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> SourcePreprocQueryContexts {
    let relevant = db.source_preproc_contexts_for_file(file_id);
    let mut file_ids = UniqVec::<FileId, FileId>::default();
    let profile_id = db.file_compilation_profile(file_id);
    let plan = db.compilation_plan_for_profile(profile_id);
    let is_include_only = plan.include_only.contains(&file_id);
    let include_self = match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog if !is_include_only => true,
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            relevant.model_file_ids.is_empty()
        }
        _ => false,
    };
    if include_self {
        file_ids.push_unique(file_id);
    }
    for model_file_id in relevant.model_file_ids.iter().copied() {
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

/// Iterates the model files covering a query file, collecting per-model errors
/// into a first-error slot. The caller's `f` may use `?` freely: the error is
/// recorded and iteration continues with the next model, matching the
/// per-context degradation semantics of preproc queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preproc) struct ContextQuery {
    contexts: SourcePreprocQueryContexts,
    first_error: Option<PreprocError>,
}

impl ContextQuery {
    pub(in crate::preproc) fn new(db: &dyn PreprocDb, file_id: FileId) -> Self {
        Self { contexts: source_preproc_single_query_contexts(db, file_id), first_error: None }
    }

    pub(in crate::preproc) fn for_each_model(
        &mut self,
        db: &dyn PreprocDb,
        mut f: impl FnMut(FileId, &MappedSourcePreprocModel) -> PreprocResult<()>,
    ) {
        for model_file_id in self.contexts.model_file_ids.iter().copied() {
            let mapped = db.source_preproc_model(model_file_id);
            let mapped = match mapped_result(mapped.as_ref()) {
                Ok(mapped) => mapped,
                Err(error) => {
                    record_first_error(&mut self.first_error, error);
                    continue;
                }
            };
            if let Err(error) = f(model_file_id, mapped) {
                record_first_error(&mut self.first_error, error);
            }
        }
    }

    /// Applies the empty-result error policy: recorded errors surface only
    /// when the query produced no results.
    pub(in crate::preproc) fn finish_empty(self, has_result: bool) -> PreprocResult<()> {
        if has_result {
            return Ok(());
        }
        finish_empty_single_query(&self.contexts, self.first_error)
    }
}

pub(in crate::preproc) trait PreprocSingleExt<T> {
    fn into_single_or_none<F>(self, ambiguous: F) -> PreprocResult<Option<T>>
    where
        F: FnOnce(usize) -> PreprocError;
}

impl<T> PreprocSingleExt<T> for Vec<T> {
    fn into_single_or_none<F>(mut self, ambiguous: F) -> PreprocResult<Option<T>>
    where
        F: FnOnce(usize) -> PreprocError,
    {
        match self.len() {
            0 => Ok(None),
            1 => Ok(self.pop()),
            contexts => Err(ambiguous(contexts)),
        }
    }
}
