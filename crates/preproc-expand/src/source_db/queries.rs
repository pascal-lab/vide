use super::*;
use crate::macro_file::SourceEmittedTokenId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourcePreprocQueryError {
    UnsupportedFileKind(SourceFileKind),
    TraceUnavailable,
    Model(SourcePreprocError),
    /// A source range could not be mapped back to a user-facing file.
    SourceUnavailable(SourcePreprocUnavailable),
    /// No source mapping exists for the source id.
    MissingSource {
        source: PreprocSourceId,
    },
    /// The mapped range exceeds the mapped file's text length.
    RangeOutOfBounds {
        source: PreprocSourceId,
        range: TextRange,
        mapped_range: TextRange,
        text_len: usize,
    },
    /// A virtual source (e.g. predefine display file) has no user-facing file.
    DisplayOnlyVirtualSource {
        path: VfsPath,
        origin: PreprocVirtualOrigin,
    },
    /// A macro expansion emitted fewer tokens than its recorded range claims.
    MissingEmittedToken {
        token: SourceEmittedTokenId,
    },
}

pub fn workspace_preproc_model_file_ids(
    db: &dyn PreprocDb,
    profile_id: Option<CompilationProfileId>,
) -> Vec<FileId> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let mut file_ids = FxHashSet::default();

    for root in plan.roots.iter().copied() {
        if matches!(
            db.file_kind(root),
            SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
        ) {
            file_ids.insert(root);
        }
    }
    file_ids.extend(plan.include_only.iter().copied());

    for source_root_id in &plan.source_roots {
        for candidate in db.source_root(*source_root_id).iter() {
            if db.file_is_project_ignored(candidate) {
                continue;
            }
            if matches!(db.file_kind(candidate), SourceFileKind::IncludeHeader) {
                file_ids.insert(candidate);
            }
        }
    }

    for candidate in db.files().iter().copied() {
        if db.file_is_project_ignored(candidate) {
            continue;
        }
        if !plan.is_include_header_in_include_paths(db, candidate) {
            continue;
        }
        file_ids.insert(candidate);
    }

    let mut file_ids = file_ids.into_iter().collect::<Vec<_>>();
    file_ids.sort();
    file_ids
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn source_preproc_model(
    db: &dyn PreprocDb,
    key: crate::db::PreprocFileQueryKey,
) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>> {
    let file_id = key.file_id(db);
    let file_kind = db.file_kind(file_id);
    if !matches!(file_kind, SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader) {
        return Arc::new(Err(SourcePreprocQueryError::UnsupportedFileKind(file_kind)));
    }

    let profile_id = db.file_compilation_profile(file_id);
    let preprocess = db.project_config().preprocess_for_profile(profile_id);
    let options = syntax_tree_options_for_file(db, file_id);
    let Some(trace) = db.preproc_trace(file_id) else {
        return Arc::new(Err(SourcePreprocQueryError::TraceUnavailable));
    };

    let source_map =
        match source_preproc_file_ids(db, file_id, profile_id, &trace, &options, &preprocess) {
            Ok(source_map) => source_map,
            Err(err) => return Arc::new(Err(err)),
        };
    let model = match SourcePreprocModel::from_trace(trace) {
        Ok(model) => model,
        Err(err) => return Arc::new(Err(SourcePreprocQueryError::Model(err))),
    };

    Arc::new(Ok(MappedSourcePreprocModel::new(model, source_map)))
}

pub(crate) fn set_source_preproc_model_lru_capacity(db: &mut dyn PreprocDb, capacity: usize) {
    source_preproc_model::set_lru_capacity(db, capacity);
}
