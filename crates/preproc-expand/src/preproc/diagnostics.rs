use super::*;

pub fn diagnostic_target_for_range(
    db: &dyn PreprocDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<DiagnosticTargetResult> {
    let mut targets = UniqVec::<DiagnosticTarget, ()>::default();
    let mut covered = false;
    let mut ambiguous_targets = 0;
    let mut query = ContextQuery::new(db, file_id);
    query.for_each_model(db, |model_file_id, mapped| {
        let source_calls = source_macro_calls_intersecting_range(mapped, file_id, range);
        match source_calls.as_slice() {
            [] => {}
            [source_call] => {
                covered = true;
                if let Some(target) =
                    diagnostic_target_for_call(db, model_file_id, mapped, source_call)?
                {
                    targets.push_unique_eq(target);
                }
            }
            source_calls => {
                covered = true;
                ambiguous_targets += source_calls.len();
            }
        }
        Ok(())
    });
    query.finish()?;

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
    Ok(DiagnosticTargetResult::uncovered())
}
