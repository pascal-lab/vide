use super::{predefines::configured_predefine_names, *};

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
                    Ok(mut definition) => {
                        definition.capability =
                            context_query_capability(&contexts, definition.capability);
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
