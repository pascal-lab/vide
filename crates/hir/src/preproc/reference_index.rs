use super::{predefines::configured_predefine_definitions_for_name, *};

pub fn macro_references(
    db: &dyn SourceRootDb,
    file_id: FileId,
    definition: &MacroDefinition,
) -> PreprocResult<MacroReferences> {
    let profile_id = db
        .file_compilation_profile(file_id)
        .or_else(|| db.file_compilation_profile(definition.file_id));
    let index = db.macro_reference_index_for_profile(profile_id);
    Ok(MacroReferences { references: index.references_for(definition), status: index.status() })
}

pub fn macro_param_references(
    db: &dyn SourceRootDb,
    file_id: FileId,
    definition: &MacroParamDefinition,
) -> PreprocResult<MacroParamReferences> {
    let profile_id = db
        .file_compilation_profile(file_id)
        .or_else(|| db.file_compilation_profile(definition.macro_definition.file_id));
    let mut references = UniqVec::<MacroParamReference, MacroParamReferenceKey>::default();
    let mut first_error = None;

    for model_file_id in workspace_preproc_model_file_ids(db, profile_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for source_definition in mapped.model.macro_definitions().iter() {
            let mapped_definition = match map_macro_definition(mapped, source_definition) {
                Ok(definition) => definition,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            if !same_macro_definition(&mapped_definition, &definition.macro_definition) {
                continue;
            }
            let Some(params) = &source_definition.params else {
                continue;
            };
            let Some(param) = params.get(definition.param_index) else {
                continue;
            };
            if param.name.as_ref() != Some(&definition.name) {
                continue;
            }

            for (token_index, token) in source_definition.body_tokens.iter().enumerate() {
                if param.name.as_ref() != Some(&token.value) {
                    continue;
                }
                let Some(token_range) = token.range else {
                    continue;
                };
                match map_macro_param_reference(
                    mapped,
                    source_definition,
                    definition.param_index,
                    token_index,
                    token_range,
                ) {
                    Ok(reference) => {
                        references.push_keyed(reference, MacroParamReferenceKey::from_reference);
                    }
                    Err(error) => record_first_error(&mut first_error, error),
                }
            }
        }
    }

    if references.is_empty()
        && let Some(error) = first_error
    {
        return Err(error);
    }

    Ok(MacroParamReferences { references: references.into_vec() })
}

pub(crate) fn build_macro_reference_index(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> MacroReferenceIndex {
    let mut index = MacroReferenceIndex::default();

    for model_file_id in workspace_preproc_model_file_ids(db, profile_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped.as_ref() {
            Ok(mapped) => mapped,
            Err(error) => {
                index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                    file_id: model_file_id,
                    error: error.clone().into(),
                });
                continue;
            }
        };
        collect_macro_references_in_model(db, mapped, model_file_id, &mut index);
    }

    index
}

fn collect_macro_references_in_model(
    db: &dyn SourceRootDb,
    mapped: &MappedSourcePreprocModel,
    model_file_id: FileId,
    index: &mut MacroReferenceIndex,
) {
    for reference in mapped.model.macro_references().iter() {
        let SourceMacroResolution::Resolved { definition, .. } = reference.resolution else {
            if reference.resolution == SourceMacroResolution::Undefined {
                collect_configured_predefine_reference(db, mapped, model_file_id, reference, index);
                continue;
            }
            if let SourceMacroResolution::Unavailable(reason) = &reference.resolution {
                index.push_issue(MacroReferenceIndexIssue::UnavailableReference {
                    file_id: model_file_id,
                    reference_id: reference.id.into(),
                    reason: PreprocUnavailable::Source(reason.clone()),
                });
            }
            continue;
        };

        let Some(definition) = mapped.model.macro_definitions().get(definition) else {
            index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                file_id: model_file_id,
                error: PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                    SourcePreprocError::MissingEvent { event_id: reference.event_id.raw() },
                )),
            });
            continue;
        };

        let definition = match map_macro_definition(mapped, definition) {
            Ok(definition) => definition,
            Err(error) => {
                index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                    file_id: model_file_id,
                    error,
                });
                continue;
            }
        };
        let reference = match map_macro_reference(mapped, reference) {
            Ok(reference) => reference,
            Err(error) => {
                index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                    file_id: model_file_id,
                    error,
                });
                continue;
            }
        };
        index.push(definition, reference);
    }
}

fn collect_configured_predefine_reference(
    db: &dyn SourceRootDb,
    mapped: &MappedSourcePreprocModel,
    model_file_id: FileId,
    source_reference: &SourceMacroReference,
    index: &mut MacroReferenceIndex,
) {
    let reference = match map_macro_reference(mapped, source_reference) {
        Ok(reference) => reference,
        Err(error) => {
            index.push_issue(MacroReferenceIndexIssue::SkippedModel {
                file_id: model_file_id,
                error,
            });
            return;
        }
    };
    for definition in configured_predefine_definitions_for_name(db, model_file_id, &reference.name)
    {
        index.push(definition, reference.clone());
    }
}
