use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocRelevantContexts {
    pub model_file_ids: Vec<FileId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocContextIndex {
    contexts_by_file: FxHashMap<FileId, Vec<FileId>>,
}

impl SourcePreprocContextIndex {
    fn contexts_for_file(&self, file_id: FileId) -> SourcePreprocRelevantContexts {
        SourcePreprocRelevantContexts {
            model_file_ids: self.contexts_by_file.get(&file_id).cloned().unwrap_or_default(),
        }
    }
}

/// Which runs read each file, inverted from what those runs actually consumed.
///
/// A run's inputs are facts, not inferences: the include edges its
/// preprocessor emitted, plus the manifest supplying its predefines. Both are
/// already memoized for other consumers, so inverting them costs one slice
/// read per root instead of a preprocessor model per root.
///
/// This shares its identity with the invalidation model. A file's dependents
/// and the runs that can answer a query about it are the same relation, so
/// they must not be two computations.
pub(crate) fn source_preproc_context_index_for_profile(
    db: &dyn PreprocDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<SourcePreprocContextIndex> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let manifest_file_ids = predefine_manifest_file_ids(db, profile_id);
    let mut contexts_by_file = FxHashMap::<FileId, Vec<FileId>>::default();

    for root in plan.root_file_ids() {
        let inputs = db.parsed_compilation_dependencies(root);
        for file_id in inputs.iter().copied().chain(manifest_file_ids.iter().copied()) {
            if file_id == root {
                continue;
            }
            contexts_by_file.entry(file_id).or_default().push(root);
        }
    }

    for roots in contexts_by_file.values_mut() {
        roots.sort_unstable_by_key(|root| root.index());
        roots.dedup();
    }
    Arc::new(SourcePreprocContextIndex { contexts_by_file })
}

/// Files whose text a profile's predefines were read from. A predefine is an
/// input to every run in the profile without being included by any of them.
fn predefine_manifest_file_ids(
    db: &dyn PreprocDb,
    profile_id: Option<CompilationProfileId>,
) -> Vec<FileId> {
    let path_file_ids = db.path_file_ids();
    let mut file_ids = db
        .project_config()
        .preprocess_for_profile(profile_id)
        .predefines
        .iter()
        .filter_map(|predefine| predefine.source.as_ref())
        .filter_map(|source| path_file_ids.get_path(source.path.as_path()))
        .collect::<Vec<_>>();
    file_ids.sort_unstable_by_key(|file_id| file_id.index());
    file_ids.dedup();
    file_ids
}

pub(crate) fn source_preproc_contexts_for_file(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> Arc<SourcePreprocRelevantContexts> {
    let profile_id = db.file_compilation_profile(file_id);
    Arc::new(db.source_preproc_context_index_for_profile(profile_id).contexts_for_file(file_id))
}
