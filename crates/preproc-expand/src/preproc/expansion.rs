use super::*;

pub fn macro_call_resolutions_in_range(
    db: &dyn PreprocDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Vec<MacroCallResolution>> {
    let mut resolutions = UniqVec::<MacroCallResolution, ()>::default();
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for source_call in source_macro_calls_intersecting_range(mapped, file_id, range) {
            let Some(reference) = mapped.model.macro_references().get(source_call.reference) else {
                continue;
            };
            let SourceMacroResolution::Resolved { definition, .. } = &reference.resolution else {
                if let SourceMacroResolution::Unavailable(reason) = &reference.resolution {
                    return Err(source_model_error(reason.clone()));
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
                return Err(PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                    SourcePreprocError::MissingEvent { event_id },
                )));
            };

            let call = map_macro_call(mapped, source_call)?;
            let definition = map_macro_definition(mapped, source_definition)?;
            resolutions.push_unique_eq(MacroCallResolution { call, definition });
        }
        Ok(())
    });
    query.finish()?;

    Ok(resolutions.into_vec())
}
