use super::*;

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
