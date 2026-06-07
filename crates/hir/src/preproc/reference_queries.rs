use super::{predefines::configured_predefine_definitions_for_name, *};

pub fn macro_usage_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroUsageResolution>> {
    macro_usage_resolutions_at(db, file_id, offset)?.into_single_or_none(|contexts| {
        PreprocUnavailable::AmbiguousMacroReferenceContexts { contexts }
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

        for reference in mapped.model.macro_references().iter() {
            let SourceMacroReferenceSite::Usage { usage_index } = reference.site else {
                continue;
            };
            match mapped_source_range_contains_provenance_offset(
                mapped,
                reference.name_range,
                file_id,
                offset,
            ) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            }

            let SourceMacroResolutionFact::Resolved { definition, include_chain, .. } =
                &reference.resolution
            else {
                if let SourceMacroResolutionFact::Unavailable(reason) = &reference.resolution {
                    unavailable_contexts += 1;
                    record_first_error(&mut first_error, unavailable_error(reason.clone()));
                }
                continue;
            };
            let mapped_reference = map_macro_reference(mapped, reference)?;
            let definition_fact =
                mapped.model.macro_definitions().get(*definition).ok_or_else(|| {
                    PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                        SourcePreprocError::MissingEvent { event_id: reference.event_id.raw() },
                    ))
                })?;
            let definition = map_macro_definition(mapped, definition_fact)?;
            let definition_provenance =
                map_definition_provenance_from_definition(mapped, definition_fact)?;
            let include_chain = map_include_chain(mapped, include_chain)?;

            resolutions.push_unique_eq(MacroUsageResolution {
                usage: MacroUsage {
                    reference_id: mapped_reference.id,
                    source: mapped_reference.source,
                    capability: mapped_reference.capability.clone(),
                    file_id: mapped_reference.file_id,
                    name: mapped_reference.name,
                    usage_index,
                    directive_range: mapped_reference.directive_range,
                    range: mapped_reference.range,
                    resolution: mapped_reference.resolution,
                },
                definition,
                definition_provenance,
                include_chain,
            });
        }
    }

    if !resolutions.is_empty() {
        return Ok(resolutions.into_vec());
    }
    if unavailable_contexts > 1 {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroReferenceContexts {
                contexts: unavailable_contexts,
            },
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
    Ok(Some(contexts.references.into_exactly_one(|contexts| {
        PreprocUnavailable::AmbiguousMacroReferenceContexts { contexts }
    })?))
}

pub fn macro_reference_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReferenceResolution>> {
    let Some(mut resolution) = macro_reference_definitions_at(db, file_id, offset)? else {
        return Ok(None);
    };
    if resolution.references.len() != 1 {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroReferenceContexts {
                contexts: resolution.references.len(),
            },
        });
    }
    let reference = resolution.references.pop().unwrap();
    let definition = resolution.definitions.into_single_or_none(|contexts| {
        PreprocUnavailable::AmbiguousMacroReferenceContexts { contexts }
    })?;
    Ok(definition.map(|definition| MacroReferenceResolution { reference, definition }))
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

        for reference in mapped.model.macro_references().iter() {
            let (_, range) = match mapped_source_range_at_offset(
                mapped,
                reference.name_range,
                file_id,
                offset,
            ) {
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
                SourceMacroResolutionFact::Resolved { definition, .. } => {
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
                SourceMacroResolutionFact::Undefined => {
                    for definition in configured_predefine_definitions_for_name(
                        db,
                        model_file_id,
                        &mapped_reference.name,
                    ) {
                        definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
                    }
                }
                SourceMacroResolutionFact::Unavailable(_) => {}
            }
        }
    }

    let Some(range) = query_range else {
        finish_empty_single_query(&contexts, first_error)?;
        return Ok(None);
    };

    Ok(Some(MacroReferenceDefinitions {
        capability: macro_reference_context_capability(references.as_slice()),
        references: references.into_vec(),
        range,
        definitions: definitions.into_vec(),
    }))
}
