use super::*;

pub fn immediate_macro_expansion_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroExpansionQuery>> {
    let mut queries = macro_expansion_queries_at(db, file_id, offset)?;
    match queries.len() {
        0 => Ok(None),
        1 => Ok(queries.pop()),
        contexts => {
            let available = queries
                .iter()
                .filter_map(|query| match query {
                    MacroExpansionQuery::Available(expansion) => Some(expansion.as_ref().clone()),
                    MacroExpansionQuery::Ambiguous(_) | MacroExpansionQuery::Unavailable(_) => None,
                })
                .collect::<Vec<_>>();
            if available.len() == contexts {
                return Ok(Some(MacroExpansionQuery::Ambiguous(available)));
            }
            Err(PreprocError::Unavailable {
                reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts },
            })
        }
    }
}

pub fn macro_expansion_queries_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroExpansionQuery>> {
    let mut queries = UniqVec::<MacroExpansionQuery, ()>::default();
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
        for call_fact in source_macro_calls_at(mapped, file_id, offset) {
            let query = immediate_macro_expansion_for_call(mapped, call_fact)?;
            queries.push_unique_eq(query);
        }
    }

    if !queries.is_empty() {
        return Ok(queries.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}

pub fn macro_call_resolutions_in_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Vec<MacroCallResolution>> {
    let mut resolutions = UniqVec::<MacroCallResolution, ()>::default();
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

        for call_fact in source_macro_calls_intersecting_range(mapped, file_id, range) {
            let SourceMacroResolution::Resolved { definition, .. } = &call_fact.callee else {
                if let SourceMacroResolution::Unavailable(reason) = &call_fact.callee {
                    record_first_error(&mut first_error, unavailable_error(reason.clone()));
                }
                continue;
            };
            let Some(definition_fact) = mapped.model.macro_definitions().get(*definition) else {
                let event_id = mapped
                    .model
                    .macro_references()
                    .get(call_fact.reference)
                    .map(|reference| reference.event_id.raw())
                    .unwrap_or_default();
                record_first_error(
                    &mut first_error,
                    PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                        SourcePreprocError::MissingEvent { event_id },
                    )),
                );
                continue;
            };

            let call = match map_macro_call(mapped, call_fact) {
                Ok(call) => call,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            let definition = match map_macro_definition(mapped, definition_fact) {
                Ok(definition) => definition,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            resolutions.push_unique_eq(MacroCallResolution { call, definition });
        }
    }

    if resolutions.is_empty()
        && let Err(error) = finish_empty_single_query(&contexts, first_error)
    {
        return Err(error);
    }

    Ok(resolutions.into_vec())
}

pub fn recursive_macro_expansion_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<RecursiveMacroExpansion>> {
    recursive_macro_expansions_at(db, file_id, offset)?.into_single_or_none(|contexts| {
        PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts }
    })
}

pub fn recursive_macro_expansions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<RecursiveMacroExpansion>> {
    let mut expansions = UniqVec::<RecursiveMacroExpansion, ()>::default();
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
        for call_fact in source_macro_calls_at(mapped, file_id, offset) {
            let recursive = recursive_macro_expansion_for_call(mapped, call_fact)?;
            expansions.push_unique_eq(recursive);
        }
    }

    if !expansions.is_empty() {
        return Ok(expansions.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}
