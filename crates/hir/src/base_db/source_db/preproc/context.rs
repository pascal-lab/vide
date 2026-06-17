use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePreprocRelevantContexts {
    pub model_file_ids: Vec<FileId>,
    pub status: SourcePreprocContextStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourcePreprocContextIndex {
    contexts_by_file: FxHashMap<FileId, Vec<FileId>>,
    status: SourcePreprocContextStatus,
}

impl SourcePreprocContextIndex {
    fn contexts_for_file(&self, file_id: FileId) -> SourcePreprocRelevantContexts {
        SourcePreprocRelevantContexts {
            model_file_ids: self.contexts_by_file.get(&file_id).cloned().unwrap_or_default(),
            status: self.status,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourcePreprocContextStatus {
    #[default]
    Complete,
    Partial {
        skipped_models: usize,
    },
}

fn preproc_context_file_ids(
    mapped: &MappedSourcePreprocModel,
    model_file_id: FileId,
) -> Vec<FileId> {
    let mut file_ids = UniqVec::<FileId, FileId>::default();
    file_ids.push_unique(model_file_id);

    for definition in mapped.model.macro_definitions().iter() {
        collect_context_source_range(mapped, definition.directive_range, &mut file_ids);
        collect_context_source_range(mapped, definition.name_range, &mut file_ids);
        if let Some(params) = &definition.params {
            for param in params {
                if let Some(range) = param.name_range {
                    collect_context_source_range(mapped, range, &mut file_ids);
                }
                if let Some(range) = param.range {
                    collect_context_source_range(mapped, range, &mut file_ids);
                }
                if let Some(default) = &param.default {
                    for token in default {
                        if let Some(range) = token.range {
                            collect_context_source_range(mapped, range, &mut file_ids);
                        }
                    }
                }
            }
        }
        for token in &definition.body_tokens {
            if let Some(range) = token.range {
                collect_context_source_range(mapped, range, &mut file_ids);
            }
        }
    }

    for reference in mapped.model.macro_references().iter() {
        collect_context_source_range(mapped, reference.directive_range, &mut file_ids);
        collect_context_source_range(mapped, reference.name_range, &mut file_ids);
    }

    for call in mapped.model.macro_calls().iter() {
        collect_context_source_range(mapped, call.call_range, &mut file_ids);
        for argument in &call.arguments {
            if let Some(range) = argument.argument_range {
                collect_context_source_range(mapped, range, &mut file_ids);
            }
            for token in &argument.tokens {
                if let Some(range) = token.range {
                    collect_context_source_range(mapped, range, &mut file_ids);
                }
            }
        }
    }

    for include in mapped.model.include_graph().directives() {
        collect_context_source_range(mapped, include.directive_range, &mut file_ids);
        if let Some(range) = include.target_range {
            collect_context_source_range(mapped, range, &mut file_ids);
        }
        if let Some(source) = include.resolved_source {
            collect_context_source(mapped, source, &mut file_ids);
        }
    }

    for range in mapped.model.inactive_ranges() {
        collect_context_source_range(mapped, *range, &mut file_ids);
    }

    for provenance in mapped.model.token_provenance().iter() {
        match provenance {
            SourceTokenProvenance::Source { token_range }
            | SourceTokenProvenance::MacroBody { body_token_range: token_range, .. } => {
                collect_context_source_range(mapped, *token_range, &mut file_ids);
            }
            SourceTokenProvenance::MacroArgument {
                body_token_range, argument_token_range, ..
            } => {
                collect_context_source_range(mapped, *body_token_range, &mut file_ids);
                collect_context_source_range(mapped, *argument_token_range, &mut file_ids);
            }
            SourceTokenProvenance::TokenPaste { .. }
            | SourceTokenProvenance::Stringification { .. }
            | SourceTokenProvenance::Builtin { .. } => {}
            SourceTokenProvenance::Predefine { source } => {
                collect_context_source(mapped, *source, &mut file_ids);
            }
        }
    }

    let mut file_ids = file_ids.into_vec();
    file_ids.sort();
    file_ids
}

fn collect_context_source_range(
    mapped: &MappedSourcePreprocModel,
    range: SourceRange,
    file_ids: &mut UniqVec<FileId, FileId>,
) {
    collect_context_source(mapped, range.source, file_ids);
}

fn collect_context_source(
    mapped: &MappedSourcePreprocModel,
    source: PreprocSourceId,
    file_ids: &mut UniqVec<FileId, FileId>,
) {
    if let Ok(file_id) = mapped.source_map.file_id(source) {
        file_ids.push_unique(file_id);
    }
    if let Some(manifest_source) = mapped.source_map.predefine_manifest_source(source) {
        file_ids.push_unique(manifest_source.file_id);
    }
}

pub(in crate::base_db::source_db) fn source_preproc_context_index_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<SourcePreprocContextIndex> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let mut contexts_by_file = FxHashMap::<FileId, UniqVec<FileId, FileId>>::default();
    let mut skipped_models = 0usize;

    for model_file_id in plan.roots.iter().copied() {
        if !matches!(
            db.file_kind(model_file_id),
            SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
        ) {
            continue;
        }
        let mapped = db.source_preproc_model(model_file_id);
        match mapped.as_ref() {
            Ok(mapped) => {
                for file_id in preproc_context_file_ids(mapped, model_file_id) {
                    if file_id == model_file_id {
                        continue;
                    }
                    contexts_by_file.entry(file_id).or_default().push_unique(model_file_id);
                }
            }
            Err(_) => skipped_models += 1,
        }
    }

    let contexts_by_file = contexts_by_file
        .into_iter()
        .map(|(file_id, model_file_ids)| {
            let mut model_file_ids = model_file_ids.into_vec();
            model_file_ids.sort();
            (file_id, model_file_ids)
        })
        .collect();
    let status = if skipped_models == 0 {
        SourcePreprocContextStatus::Complete
    } else {
        SourcePreprocContextStatus::Partial { skipped_models }
    };
    Arc::new(SourcePreprocContextIndex { contexts_by_file, status })
}

pub(in crate::base_db::source_db) fn source_preproc_contexts_for_file(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<SourcePreprocRelevantContexts> {
    let profile_id = db.file_compilation_profile(file_id);
    Arc::new(db.source_preproc_context_index_for_profile(profile_id).contexts_for_file(file_id))
}
