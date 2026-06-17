use super::*;

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

        for source_call in source_macro_calls_intersecting_range(mapped, file_id, range) {
            let SourceMacroResolution::Resolved { definition, .. } = &source_call.callee else {
                if let SourceMacroResolution::Unavailable(reason) = &source_call.callee {
                    record_first_error(&mut first_error, unavailable_error(reason.clone()));
                }
                continue;
            };
            let Some(source_definition) = mapped.model.macro_definitions().get(*definition) else {
                let event_id = mapped
                    .model
                    .macro_references()
                    .get(source_call.reference)
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

            let call = match map_macro_call(mapped, source_call) {
                Ok(call) => call,
                Err(error) => {
                    record_first_error(&mut first_error, error);
                    continue;
                }
            };
            let definition = match map_macro_definition(mapped, source_definition) {
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
