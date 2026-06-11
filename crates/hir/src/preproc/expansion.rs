use super::*;

pub fn recursive_macro_expansion_provenance_for_source_graph_call(
    db: &dyn SourceRootDb,
    model_file_id: FileId,
    call_id: source_model::MacroCallId,
) -> PreprocResult<Option<RecursiveMacroExpansionProvenance>> {
    let mapped = db.source_preproc_model(model_file_id);
    let mapped = mapped_result(mapped.as_ref())?;
    let Some(call_fact) = mapped
        .model
        .macro_calls()
        .get(preproc::source::SourceMacroCallId::new(call_id.raw() as usize))
    else {
        return Ok(None);
    };
    recursive_macro_expansion_provenance_for_call(mapped, call_fact).map(Some)
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
            let SourceMacroResolutionFact::Resolved { definition, .. } = &call_fact.callee else {
                if let SourceMacroResolutionFact::Unavailable(reason) = &call_fact.callee {
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

            let mut call = match map_macro_call(mapped, call_fact) {
                Ok(call) => call,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            let mut definition = match map_macro_definition(mapped, definition_fact) {
                Ok(definition) => definition,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            call.capability = context_query_capability(&contexts, call.capability);
            definition.capability = context_query_capability(&contexts, definition.capability);
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
