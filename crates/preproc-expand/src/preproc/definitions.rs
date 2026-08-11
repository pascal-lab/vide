use super::{
    predefines::{configured_predefine_definitions_at, configured_predefine_names},
    *,
};

pub fn visible_macros_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroDefinition>> {
    let mut definitions = UniqVec::<MacroDefinition, MacroDefinitionKey>::default();
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for position in mapped.source_map.source_positions_for_file_offset(file_id, offset) {
            for definition in mapped.model.visible_macros_at(position) {
                let definition = map_macro_definition(mapped, definition)?;
                definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
            }
        }
        Ok(())
    });
    query.finish()?;

    Ok(definitions.into_vec())
}

pub fn visible_macro_names_at(
    db: &dyn PreprocDb,
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
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroDefinition>> {
    let mut first = None;
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for definition_id in mapped.macro_definition_ids_at(file_id, offset) {
            let Some(definition) = mapped.model.macro_definitions().get(definition_id) else {
                continue;
            };
            let mapped_definition = map_macro_definition(mapped, definition)?;
            if mapped_definition.name_range.contains(offset) {
                first = Some(mapped_definition);
                break;
            }
        }
        Ok(())
    });

    if first.is_some() {
        return Ok(first);
    }

    if let Some(definition) = configured_predefine_definitions_at(db, file_id, offset)?
        .into_single_or_none(|contexts| PreprocError::Ambiguous {
            kind: AmbiguousKind::MacroDefinition,
            count: contexts,
        })?
    {
        return Ok(Some(definition));
    }

    query.finish()?;

    Ok(None)
}

pub fn macro_param_definition_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroParamDefinition>> {
    macro_param_definitions_at(db, file_id, offset)?.into_single_or_none(|contexts| {
        PreprocError::Ambiguous { kind: AmbiguousKind::MacroParam, count: contexts }
    })
}

pub fn macro_param_definitions_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroParamDefinition>> {
    let mut definitions = UniqVec::<MacroParamDefinition, MacroParamDefinitionKey>::default();
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for (definition_id, param_index) in mapped.macro_param_definition_ids_at(file_id, offset) {
            let Some(definition) = mapped.model.macro_definitions().get(definition_id) else {
                continue;
            };
            let Some(params) = &definition.params else {
                continue;
            };
            let Some(param) = params.get(param_index) else {
                continue;
            };
            let Some(param_definition) =
                map_macro_param_definition(mapped, definition, param_index, param)?
            else {
                continue;
            };
            if param_definition.range.contains(offset) {
                definitions.push_keyed(param_definition, MacroParamDefinitionKey::from_definition);
            }
        }
        Ok(())
    });
    query.finish()?;

    Ok(definitions.into_vec())
}

pub fn macro_param_reference_definitions_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroParamReferenceDefinitions>> {
    let mut definitions = UniqVec::<MacroParamDefinition, MacroParamDefinitionKey>::default();
    let mut references = UniqVec::<MacroParamReference, MacroParamReferenceKey>::default();
    let mut query_range = None;
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for (definition_id, token_index) in mapped.macro_param_reference_ids_at(file_id, offset) {
            let Some(definition) = mapped.model.macro_definitions().get(definition_id) else {
                continue;
            };
            let Some(params) = &definition.params else {
                continue;
            };
            let Some(token) = definition.body_tokens.get(token_index) else {
                continue;
            };
            let Some(token_range) = token.range else {
                continue;
            };
            let Some((_, range)) =
                source_mapping_range_at_offset(mapped, token_range, file_id, offset)?
            else {
                continue;
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
                definitions.push_keyed(param_definition, MacroParamDefinitionKey::from_definition);
                references.push_keyed(reference, MacroParamReferenceKey::from_reference);
            }
        }
        Ok(())
    });

    let Some(range) = query_range else {
        query.finish()?;
        return Ok(None);
    };

    let references = references.into_vec();
    let definitions = definitions.into_vec();
    Ok(Some(MacroParamReferenceDefinitions { references, range, definitions }))
}
