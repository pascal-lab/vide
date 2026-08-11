use super::{predefines::configured_predefine_definitions_for_name, *};

pub fn macro_references_in_range(
    db: &dyn PreprocDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Vec<MacroReference>> {
    let mut references = UniqVec::<MacroReference, ()>::default();
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for reference_id in mapped.macro_reference_ids_intersecting_range(file_id, range) {
            let Some(reference) = mapped.model.macro_references().get(reference_id) else {
                continue;
            };
            references.push_unique_eq(map_macro_reference(mapped, reference)?);
        }
        Ok(())
    });
    query.finish()?;

    Ok(references.into_vec())
}

pub fn macro_reference_definitions_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReferenceDefinitions>> {
    let mut definitions = UniqVec::<MacroDefinition, MacroDefinitionKey>::default();
    let mut references = UniqVec::<MacroReference, ()>::default();
    let mut query_range = None;
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |model_file_id, mapped| {
        for reference_id in mapped.macro_reference_ids_at(file_id, offset) {
            let Some(reference) = mapped.model.macro_references().get(reference_id) else {
                continue;
            };
            let Some((_, range)) =
                source_mapping_range_at_offset(mapped, reference.name_range, file_id, offset)?
            else {
                continue;
            };
            query_range.get_or_insert(range);

            let mapped_reference = map_macro_reference(mapped, reference)?;
            references.push_unique_eq(mapped_reference.clone());

            match &reference.resolution {
                SourceMacroResolution::Resolved { definition, .. } => {
                    let Some(definition) = mapped.model.macro_definitions().get(*definition) else {
                        return Err(PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                            SourcePreprocError::MissingEvent { event_id: reference.event_id.raw() },
                        )));
                    };
                    let definition = map_macro_definition(mapped, definition)?;
                    definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
                }
                SourceMacroResolution::Undefined => {
                    for definition in configured_predefine_definitions_for_name(
                        db,
                        model_file_id,
                        &mapped_reference.name,
                    ) {
                        definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
                    }
                }
                SourceMacroResolution::Unavailable(_) => {}
            }
        }
        Ok(())
    });

    let Some(range) = query_range else {
        query.finish()?;
        return Ok(None);
    };

    Ok(Some(MacroReferenceDefinitions {
        references: references.into_vec(),
        range,
        definitions: definitions.into_vec(),
    }))
}
