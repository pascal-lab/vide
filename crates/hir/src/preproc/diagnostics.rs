use super::*;

pub fn diagnostic_provenance_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Option<DiagnosticProvenance>> {
    let mut provenances = UniqVec::<DiagnosticProvenance, ()>::default();
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
        let call_facts = source_macro_calls_intersecting_range(mapped, file_id, range);
        match call_facts.as_slice() {
            [] => continue,
            [call_fact] => {
                let provenance = diagnostic_provenance_for_call(mapped, call_fact)?;
                provenances.push_unique_eq(provenance);
            }
            call_facts => {
                ambiguous_targets += call_facts.len();
            }
        }
    }

    let precise = provenances
        .as_slice()
        .iter()
        .filter(|provenance| !matches!(provenance, DiagnosticProvenance::Unavailable(_)))
        .cloned()
        .collect::<Vec<_>>();
    if ambiguous_targets > 0 {
        return Ok(Some(DiagnosticProvenance::Unavailable(
            PreprocUnavailable::AmbiguousDiagnosticProvenance {
                targets: ambiguous_targets + precise.len(),
            },
        )));
    }
    if precise.len() == 1 {
        return Ok(Some(precise.into_iter().next().unwrap()));
    }
    if precise.len() > 1 {
        return Ok(Some(DiagnosticProvenance::Unavailable(
            PreprocUnavailable::AmbiguousDiagnosticProvenance { targets: precise.len() },
        )));
    }
    if provenances.len() == 1 {
        return Ok(provenances.into_vec().into_iter().next());
    }
    if provenances.len() > 1 {
        return Ok(Some(DiagnosticProvenance::Unavailable(
            PreprocUnavailable::AmbiguousDiagnosticProvenance { targets: provenances.len() },
        )));
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(None)
}
