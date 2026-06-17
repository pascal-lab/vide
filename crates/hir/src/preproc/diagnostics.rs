use super::*;

pub fn diagnostic_target_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<DiagnosticTargetResult> {
    let mut targets = UniqVec::<DiagnosticTarget, ()>::default();
    let mut covered = false;
    let mut ambiguous_targets = 0;
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
        let source_calls = source_macro_calls_intersecting_range(mapped, file_id, range);
        match source_calls.as_slice() {
            [] => continue,
            [source_call] => {
                covered = true;
                if let Some(target) = diagnostic_target_for_call(mapped, source_call)? {
                    targets.push_unique_eq(target);
                }
            }
            source_calls => {
                covered = true;
                ambiguous_targets += source_calls.len();
            }
        }
    }

    if ambiguous_targets > 0 {
        return Ok(DiagnosticTargetResult::covered(None));
    }
    if targets.len() == 1 {
        return Ok(DiagnosticTargetResult::covered(targets.into_vec().into_iter().next()));
    }
    if targets.len() > 1 {
        return Ok(DiagnosticTargetResult::covered(None));
    }
    if covered {
        return Ok(DiagnosticTargetResult::covered(None));
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(DiagnosticTargetResult::uncovered())
}
