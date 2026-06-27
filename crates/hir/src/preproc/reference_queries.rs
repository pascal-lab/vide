use super::{predefines::configured_predefine_definitions_for_name, *};

pub fn macro_usage_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroUsageResolution>> {
    macro_usage_resolutions_at(db, file_id, offset)?.into_single_or_none(|contexts| {
        PreprocError::Ambiguous { kind: AmbiguousKind::MacroReference, count: contexts }
    })
}

pub fn macro_usage_resolutions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroUsageResolution>> {
    let mut resolutions = UniqVec::<MacroUsageResolution, ()>::default();
    let mut first_error = None;
    let mut unavailable_contexts = 0;
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

        for reference_id in mapped.macro_reference_ids_at(file_id, offset) {
            let Some(reference) = mapped.model.macro_references().get(reference_id) else {
                continue;
            };
            let SourceMacroReferenceSite::Usage { .. } = reference.site else {
                continue;
            };

            let SourceMacroResolution::Resolved { definition, include_chain, .. } =
                &reference.resolution
            else {
                if let SourceMacroResolution::Unavailable(reason) = &reference.resolution {
                    unavailable_contexts += 1;
                    record_first_error(&mut first_error, source_model_error(reason.clone()));
                }
                continue;
            };
            let (usage_file_id, range) = map_reference_ranges(mapped, reference)?;
            let source_definition =
                mapped.model.macro_definitions().get(*definition).ok_or_else(|| {
                    PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                        SourcePreprocError::MissingEvent { event_id: reference.event_id.raw() },
                    ))
                })?;
            let definition = map_macro_definition(mapped, source_definition)?;
            let include_chain = map_include_chain(mapped, include_chain)?;

            resolutions.push_unique_eq(MacroUsageResolution {
                usage: MacroUsage { file_id: usage_file_id, range },
                definition,
                include_chain,
            });
        }
    }

    if !resolutions.is_empty() {
        return Ok(resolutions.into_vec());
    }
    if unavailable_contexts > 1 {
        return Err(PreprocError::Ambiguous {
            kind: AmbiguousKind::MacroReference,
            count: unavailable_contexts,
        });
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}

pub fn macro_reference_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReference>> {
    let Some(contexts) = macro_reference_definitions_at(db, file_id, offset)? else {
        return Ok(None);
    };
    Ok(Some(contexts.references.into_exactly_one(|contexts| PreprocError::Ambiguous {
        kind: AmbiguousKind::MacroReference,
        count: contexts,
    })?))
}

pub fn macro_references_in_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Vec<MacroReference>> {
    let mut references = UniqVec::<MacroReference, ()>::default();
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

        for reference_id in mapped.macro_reference_ids_intersecting_range(file_id, range) {
            let Some(reference) = mapped.model.macro_references().get(reference_id) else {
                continue;
            };

            match map_macro_reference(mapped, reference) {
                Ok(reference) => {
                    references.push_unique_eq(reference);
                }
                Err(error) => record_first_error(&mut first_error, error),
            }
        }
    }

    if references.is_empty()
        && let Err(error) = finish_empty_single_query(&contexts, first_error)
    {
        return Err(error);
    }

    Ok(references.into_vec())
}

pub fn macro_reference_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReferenceDefinitions>> {
    let mut definitions = UniqVec::<MacroDefinition, MacroDefinitionKey>::default();
    let mut references = UniqVec::<MacroReference, ()>::default();
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

        for reference_id in mapped.macro_reference_ids_at(file_id, offset) {
            let Some(reference) = mapped.model.macro_references().get(reference_id) else {
                continue;
            };
            let (_, range) =
                match source_mapping_range_at_offset(mapped, reference.name_range, file_id, offset)
                {
                    Ok(Some(hit)) => hit,
                    Ok(None) => continue,
                    Err(error) => {
                        record_first_error(&mut first_error, error);
                        continue;
                    }
                };
            query_range.get_or_insert(range);

            let mapped_reference = match map_macro_reference(mapped, reference) {
                Ok(reference) => reference,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            references.push_unique_eq(mapped_reference.clone());

            match &reference.resolution {
                SourceMacroResolution::Resolved { definition, .. } => {
                    let Some(definition) = mapped.model.macro_definitions().get(*definition) else {
                        record_first_error(
                            &mut first_error,
                            PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                                SourcePreprocError::MissingEvent {
                                    event_id: reference.event_id.raw(),
                                },
                            )),
                        );
                        continue;
                    };
                    let definition = match map_macro_definition(mapped, definition) {
                        Ok(definition) => definition,
                        Err(error) => {
                            record_first_error(&mut first_error, error);
                            continue;
                        }
                    };

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
    }

    let Some(range) = query_range else {
        finish_empty_single_query(&contexts, first_error)?;
        return Ok(None);
    };

    Ok(Some(MacroReferenceDefinitions {
        references: references.into_vec(),
        range,
        definitions: definitions.into_vec(),
    }))
}
