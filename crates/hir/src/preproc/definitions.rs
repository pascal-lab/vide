use super::{
    predefines::{configured_predefine_definitions_at, configured_predefine_names},
    *,
};

pub fn visible_macros_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroDefinition>> {
    let mut definitions = UniqVec::<MacroDefinition, MacroDefinitionKey>::default();
    let mut first_error = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);
    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for position in mapped.source_map.source_positions_for_file_offset(file_id, offset) {
            for definition in mapped.model.visible_macros_at(position) {
                match map_macro_definition(mapped, definition) {
                    Ok(definition) => {
                        definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
                    }
                    Err(error) => record_first_error(&mut first_error, error),
                }
            }
        }
    }

    if definitions.is_empty()
        && let Err(error) = finish_empty_single_query(&contexts, first_error)
    {
        return Err(error);
    }

    Ok(definitions.into_vec())
}

pub fn visible_macro_names_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<SmolStr>> {
    let mut names = UniqVec::<SmolStr, SmolStr>::default();
    for definition in visible_macros_at(db, file_id, offset)? {
        names.push_unique(definition.name.clone());
    }
    for name in configured_predefine_names(db, file_id) {
        names.push_unique(name);
    }

    Ok(names.into_vec())
}

pub fn macro_definition_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroDefinition>> {
    let mut first_error = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);
    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for definition in mapped.model.macro_definitions().iter() {
            let mapped_definition = map_macro_definition(mapped, definition)?;
            if mapped_definition.file_id == file_id && mapped_definition.name_range.contains(offset)
            {
                return Ok(Some(mapped_definition));
            }
        }
    }

    if let Some(definition) = configured_predefine_definitions_at(db, file_id, offset)?
        .into_single_or_none(|contexts| PreprocUnavailable::AmbiguousMacroDefinitionContexts {
            contexts,
        })?
    {
        return Ok(Some(definition));
    }

    finish_empty_single_query(&contexts, first_error)?;

    Ok(None)
}

pub fn macro_param_definition_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroParamDefinition>> {
    macro_param_definitions_at(db, file_id, offset)?.into_single_or_none(|contexts| {
        PreprocUnavailable::AmbiguousMacroParamContexts { contexts }
    })
}

pub fn macro_param_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroParamDefinition>> {
    let mut definitions = UniqVec::<MacroParamDefinition, MacroParamDefinitionKey>::default();
    let mut first_error = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);

    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for definition in mapped.model.macro_definitions().iter() {
            let Some(params) = &definition.params else {
                continue;
            };
            for (param_index, param) in params.iter().enumerate() {
                let Some(param_definition) =
                    map_macro_param_definition(mapped, definition, param_index, param)?
                else {
                    continue;
                };
                if param_definition.macro_definition.file_id == file_id
                    && param_definition.range.contains(offset)
                {
                    definitions
                        .push_keyed(param_definition, MacroParamDefinitionKey::from_definition);
                }
            }
        }
    }

    if definitions.is_empty()
        && let Err(error) = finish_empty_single_query(&contexts, first_error)
    {
        return Err(error);
    }

    Ok(definitions.into_vec())
}

pub fn macro_param_reference_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroParamReferenceDefinitions>> {
    let mut definitions = UniqVec::<MacroParamDefinition, MacroParamDefinitionKey>::default();
    let mut references = UniqVec::<MacroParamReference, MacroParamReferenceKey>::default();
    let mut query_range = None;
    let mut first_error = None;
    let contexts = source_preproc_single_query_contexts(db, file_id);

    for model_file_id in contexts.model_file_ids.iter().copied() {
        let mapped = db.source_preproc_model(model_file_id);
        let mapped = match mapped_result(mapped.as_ref()) {
            Ok(mapped) => mapped,
            Err(error) => {
                record_first_error(&mut first_error, error);
                continue;
            }
        };

        for definition in mapped.model.macro_definitions().iter() {
            let Some(params) = &definition.params else {
                continue;
            };
            for (token_index, token) in definition.body_tokens.iter().enumerate() {
                let Some(token_range) = token.range else {
                    continue;
                };
                let (_, range) =
                    match mapped_source_range_at_offset(mapped, token_range, file_id, offset) {
                        Ok(Some(hit)) => hit,
                        Ok(None) => continue,
                        Err(error) => {
                            record_first_error(&mut first_error, error);
                            continue;
                        }
                    };

                for (param_index, param) in params.iter().enumerate() {
                    if param.name.as_ref() != Some(&token.value) {
                        continue;
                    }
                    let Some(param_definition) =
                        map_macro_param_definition(mapped, definition, param_index, param)?
                    else {
                        continue;
                    };
                    let reference = map_macro_param_reference(
                        mapped,
                        definition,
                        param_index,
                        token_index,
                        token_range,
                    )?;
                    query_range.get_or_insert(range);
                    definitions
                        .push_keyed(param_definition, MacroParamDefinitionKey::from_definition);
                    references.push_keyed(reference, MacroParamReferenceKey::from_reference);
                }
            }
        }
    }

    let Some(range) = query_range else {
        finish_empty_single_query(&contexts, first_error)?;
        return Ok(None);
    };

    let references = references.into_vec();
    let definitions = definitions.into_vec();
    Ok(Some(MacroParamReferenceDefinitions { references, range, definitions }))
}
