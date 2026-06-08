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
            let mut query = immediate_macro_expansion_for_call(mapped, call_fact)?;
            apply_context_capability_to_macro_expansion_query(&contexts, &mut query);
            queries.push_unique_eq(query);
        }
    }

    if !queries.is_empty() {
        return Ok(queries.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
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
            let mut recursive = recursive_macro_expansion_for_call(mapped, call_fact)?;
            apply_context_capability_to_recursive_macro_expansion(&contexts, &mut recursive);
            expansions.push_unique_eq(recursive);
        }
    }

    if !expansions.is_empty() {
        return Ok(expansions.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}

pub fn recursive_macro_expansion_provenances_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<RecursiveMacroExpansionProvenance>> {
    let mut expansions = UniqVec::<RecursiveMacroExpansionProvenance, ()>::default();
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
            let mut recursive = recursive_macro_expansion_provenance_for_call(mapped, call_fact)?;
            apply_context_capability_to_recursive_macro_expansion_provenance(
                &contexts,
                &mut recursive,
            );
            expansions.push_unique_eq(recursive);
        }
    }

    if !expansions.is_empty() {
        return Ok(expansions.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}

pub fn macro_expansion_provenance_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroExpansionProvenance>> {
    macro_expansion_provenances_at(db, file_id, offset)?.into_single_or_none(|contexts| {
        PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts }
    })
}

pub fn macro_expansion_provenances_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroExpansionProvenance>> {
    let mut provenances = UniqVec::<MacroExpansionProvenance, ()>::default();
    let mut unavailable = Vec::new();
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
            match macro_expansion_provenance_for_call(mapped, call_fact)? {
                MacroExpansionProvenanceForCall::Available(provenance) => {
                    let mut provenance = *provenance;
                    apply_context_capability_to_macro_expansion_provenance(
                        &contexts,
                        &mut provenance,
                    );
                    provenances.push_unique_eq(provenance);
                }
                MacroExpansionProvenanceForCall::Unavailable(reason) => unavailable.push(reason),
            }
        }
    }

    if !unavailable.is_empty() {
        return unavailable_or_ambiguous_macro_expansion_provenance(provenances.len(), unavailable);
    }
    if !provenances.is_empty() {
        return Ok(provenances.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}

pub fn macro_expansion_provenance_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Option<MacroExpansionProvenance>> {
    macro_expansion_provenances_for_range(db, file_id, range)?.into_single_or_none(|contexts| {
        PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts }
    })
}

pub fn macro_expansion_provenances_for_range(
    db: &dyn SourceRootDb,
    file_id: FileId,
    range: TextRange,
) -> PreprocResult<Vec<MacroExpansionProvenance>> {
    let mut provenances = UniqVec::<MacroExpansionProvenance, ()>::default();
    let mut unavailable = Vec::new();
    let mut ambiguous_contexts = 0;
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
            [call_fact] => match macro_expansion_provenance_for_call(mapped, call_fact)? {
                MacroExpansionProvenanceForCall::Available(provenance) => {
                    let mut provenance = *provenance;
                    apply_context_capability_to_macro_expansion_provenance(
                        &contexts,
                        &mut provenance,
                    );
                    provenances.push_unique_eq(provenance);
                }
                MacroExpansionProvenanceForCall::Unavailable(reason) => unavailable.push(reason),
            },
            call_facts => {
                ambiguous_contexts += call_facts.len();
            }
        }
    }

    if ambiguous_contexts > 0 {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::AmbiguousMacroExpansionContexts {
                contexts: ambiguous_contexts + provenances.len() + unavailable.len(),
            },
        });
    }
    if !unavailable.is_empty() {
        return unavailable_or_ambiguous_macro_expansion_provenance(provenances.len(), unavailable);
    }
    if !provenances.is_empty() {
        return Ok(provenances.into_vec());
    }
    finish_empty_single_query(&contexts, first_error)?;

    Ok(Vec::new())
}

fn unavailable_or_ambiguous_macro_expansion_provenance(
    available_contexts: usize,
    mut unavailable: Vec<PreprocUnavailable>,
) -> PreprocResult<Vec<MacroExpansionProvenance>> {
    let contexts = available_contexts + unavailable.len();
    if contexts == 1 {
        return Err(PreprocError::Unavailable { reason: unavailable.pop().unwrap() });
    }
    Err(PreprocError::Unavailable {
        reason: PreprocUnavailable::AmbiguousMacroExpansionContexts { contexts },
    })
}

fn apply_context_capability_to_macro_call(
    contexts: &SourcePreprocQueryContexts,
    call: &mut MacroCall,
) {
    call.capability = context_query_capability(contexts, call.capability.clone());
}

fn apply_context_capability_to_macro_expansion(
    contexts: &SourcePreprocQueryContexts,
    expansion: &mut MacroExpansion,
) {
    apply_context_capability_to_macro_call(contexts, &mut expansion.call);
    let definition_capability =
        context_query_capability(contexts, expansion.definition.capability().clone());
    *expansion.definition.capability_mut() = definition_capability;
    expansion.capability = context_query_capability(contexts, expansion.capability.clone());
}

fn apply_context_capability_to_macro_expansion_unavailable(
    contexts: &SourcePreprocQueryContexts,
    unavailable: &mut MacroExpansionUnavailable,
) {
    apply_context_capability_to_macro_call(contexts, &mut unavailable.call);
}

fn apply_context_capability_to_macro_expansion_query(
    contexts: &SourcePreprocQueryContexts,
    query: &mut MacroExpansionQuery,
) {
    match query {
        MacroExpansionQuery::Available(expansion) => {
            apply_context_capability_to_macro_expansion(contexts, expansion);
        }
        MacroExpansionQuery::Ambiguous(expansions) => {
            for expansion in expansions {
                apply_context_capability_to_macro_expansion(contexts, expansion);
            }
        }
        MacroExpansionQuery::Unavailable(unavailable) => {
            apply_context_capability_to_macro_expansion_unavailable(contexts, unavailable);
        }
    }
}

fn apply_context_capability_to_recursive_macro_expansion(
    contexts: &SourcePreprocQueryContexts,
    recursive: &mut RecursiveMacroExpansion,
) {
    apply_context_capability_to_macro_call(contexts, &mut recursive.root_call);
    for expansion in &mut recursive.expansions {
        apply_context_capability_to_macro_expansion(contexts, expansion);
    }
    for unavailable in &mut recursive.unavailable {
        apply_context_capability_to_macro_expansion_unavailable(contexts, unavailable);
    }
}

fn apply_context_capability_to_recursive_macro_expansion_provenance(
    contexts: &SourcePreprocQueryContexts,
    recursive: &mut RecursiveMacroExpansionProvenance,
) {
    apply_context_capability_to_macro_call(contexts, &mut recursive.root_call);
    for expansion in &mut recursive.expansions {
        apply_context_capability_to_macro_expansion_provenance(contexts, expansion);
    }
    for unavailable in &mut recursive.unavailable {
        apply_context_capability_to_macro_expansion_unavailable(contexts, unavailable);
    }
}

fn apply_context_capability_to_macro_expansion_provenance(
    contexts: &SourcePreprocQueryContexts,
    provenance: &mut MacroExpansionProvenance,
) {
    apply_context_capability_to_macro_expansion(contexts, &mut provenance.expansion);
    for token in &mut provenance.tokens {
        match &mut token.provenance {
            TokenProvenance::MacroBody { call, .. }
            | TokenProvenance::MacroArgument { call, .. }
            | TokenProvenance::Builtin { call, .. } => {
                apply_context_capability_to_macro_call(contexts, call);
            }
            TokenProvenance::SourceToken { .. }
            | TokenProvenance::Predefine { .. }
            | TokenProvenance::Unavailable(_) => {}
        }
    }
}

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
