use super::*;

pub fn include_directive_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<IncludeDirective>> {
    include_directives_at(db, file_id, offset)?
        .into_single_or_none(|targets| PreprocError::AmbiguousIncludeTargets { targets })
}

pub fn include_directives_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<IncludeDirective>> {
    let mut directives = UniqVec::<IncludeDirective, ()>::default();
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
        for include in mapped.model.include_graph().directives() {
            let Some(target_range) = include.target_range else {
                continue;
            };
            let (_, range) =
                match source_mapping_range_at_offset(mapped, target_range, file_id, offset) {
                    Ok(Some(hit)) => hit,
                    Ok(None) => continue,
                    Err(error) => {
                        record_first_error(&mut first_error, error);
                        continue;
                    }
                };
            let resolved_file = map_include_resolved_file(mapped, &include.status)?;
            let target = match &include.target {
                MacroIncludeTarget::Literal { path, .. } => {
                    IncludeTarget::Literal { path: path.clone(), resolved_file }
                }
                MacroIncludeTarget::Token { raw } => IncludeTarget::Token { raw: raw.clone() },
            };
            let directive = IncludeDirective { id: include.id, file_id, range, target };
            directives.push_unique_by(directive, |existing, directive| {
                existing.file_id == directive.file_id
                    && existing.range == directive.range
                    && existing.target == directive.target
            });
        }
    }

    if !directives.is_empty() {
        return Ok(directives.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}
