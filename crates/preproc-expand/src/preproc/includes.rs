use super::*;

pub fn include_directive_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<IncludeDirective>> {
    include_directives_at(db, file_id, offset)?.into_single_or_none(|targets| {
        PreprocError::Ambiguous { kind: AmbiguousKind::IncludeTarget, count: targets }
    })
}

pub fn include_directives_at(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<IncludeDirective>> {
    let mut directives = UniqVec::<IncludeDirective, ()>::default();
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |_model_file_id, mapped| {
        for include in mapped.model.include_graph().directives() {
            let Some(target_range) = include.target_range else {
                continue;
            };
            let Some((_, range)) =
                source_mapping_range_at_offset(mapped, target_range, file_id, offset)?
            else {
                continue;
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
        Ok(())
    });
    query.finish_empty(!directives.is_empty())?;

    Ok(directives.into_vec())
}
