use super::*;

pub(in crate::preproc) fn diagnostic_provenance_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCallFact,
) -> PreprocResult<DiagnosticProvenance> {
    match mapped.model.immediate_macro_expansion(call_fact.id) {
        SourceMacroExpansionQueryFact::Available(expansion_id) => {
            let Some(expansion) = mapped.model.macro_expansions().get(expansion_id) else {
                return Ok(DiagnosticProvenance::Unavailable(PreprocUnavailable::Source(
                    SourcePreprocUnavailable::MissingMacroExpansion { call: call_fact.id },
                )));
            };
            diagnostic_target_for_source_expansion(mapped, expansion)
        }
        SourceMacroExpansionQueryFact::Unavailable(reason) => {
            Ok(DiagnosticProvenance::Unavailable(PreprocUnavailable::Source(reason)))
        }
    }
}

pub(in crate::preproc) fn emitted_token_ids(
    range: SourceEmittedTokenRange,
) -> impl Iterator<Item = SourceEmittedTokenId> {
    let start = range.start.raw();
    let end = start.saturating_add(range.len);
    (start..end).map(SourceEmittedTokenId::new)
}

pub(in crate::preproc) fn diagnostic_provenance_for_token(
    mapped: &MappedSourcePreprocModel,
    provenance: &SourceTokenProvenance,
) -> PreprocResult<Option<DiagnosticProvenance>> {
    Ok(match provenance {
        SourceTokenProvenance::Source { token_range } => {
            let (source, range) = map_mapped_source_range(mapped, *token_range)?;
            Some(DiagnosticProvenance::SourceToken { source, range })
        }
        SourceTokenProvenance::MacroBody { definition, body_token_range, call, .. } => {
            let call = mapped_macro_call(mapped, *call)?;
            let (source, range) = map_mapped_source_range(mapped, *body_token_range)?;
            Some(DiagnosticProvenance::MacroBody {
                call,
                definition_id: (*definition).into(),
                source,
                range,
            })
        }
        SourceTokenProvenance::MacroArgument {
            call, argument_index, argument_token_range, ..
        } => {
            let call = mapped_macro_call(mapped, *call)?;
            let Ok((source, range)) = map_mapped_source_range(mapped, *argument_token_range) else {
                return Ok(Some(expansion_authority_unavailable()));
            };
            Some(DiagnosticProvenance::MacroArgument {
                call,
                argument_index: *argument_index,
                source,
                range,
            })
        }
        SourceTokenProvenance::TokenPaste { call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            Some(expansion_authority_unavailable())
        }
        SourceTokenProvenance::Stringification { call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            Some(expansion_authority_unavailable())
        }
        SourceTokenProvenance::Predefine { source } => {
            let _source = map_mapped_source_id(mapped, *source)?;
            None
        }
        SourceTokenProvenance::Builtin { name, call, .. } => Some(DiagnosticProvenance::Builtin {
            name: name.clone(),
            call: mapped_macro_call(mapped, *call)?,
        }),
        SourceTokenProvenance::Unavailable(_) => Some(expansion_authority_unavailable()),
    })
}

pub(in crate::preproc) fn mapped_macro_call(
    mapped: &MappedSourcePreprocModel,
    call: SourceMacroCallId,
) -> PreprocResult<MacroCall> {
    let Some(call) = mapped.model.macro_calls().get(call) else {
        return Err(unavailable_error(SourcePreprocUnavailable::MissingMacroCall { call }));
    };
    map_macro_call(mapped, call)
}

pub(in crate::preproc) fn diagnostic_target_for_source_expansion(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansionFact,
) -> PreprocResult<DiagnosticProvenance> {
    let mut saw_unavailable = None;
    for token_id in emitted_token_ids(expansion.emitted_token_range) {
        let Some(token) = mapped.model.emitted_tokens().get(token_id) else {
            return Err(PreprocError::SourceMap(PreprocSourceMapError::MissingEmittedToken {
                token: token_id,
            }));
        };
        let Some(provenance) = mapped.model.token_provenance().get(token.provenance) else {
            return Err(unavailable_error(SourcePreprocUnavailable::ExpansionAuthorityUnavailable));
        };
        match diagnostic_provenance_for_token(mapped, provenance)? {
            Some(DiagnosticProvenance::Unavailable(reason)) => {
                saw_unavailable = Some(reason);
            }
            Some(provenance) => return Ok(provenance),
            None => {}
        }
    }

    if let Some(reason) = saw_unavailable {
        return Ok(DiagnosticProvenance::Unavailable(reason));
    }

    let source_buffer_source = map_expansion_source_buffer(mapped, expansion.id)?;
    let MappedPreprocSource::VirtualFile { .. } = &source_buffer_source else {
        return Ok(DiagnosticProvenance::Unavailable(display_only_virtual_expansion_unavailable(
            &source_buffer_source,
        )));
    };
    let source_buffer_range = mapped
        .source_map
        .emitted_source_buffer_range(expansion.id, expansion.emitted_token_range)
        .map_err(PreprocError::SourceMap)?;
    Ok(DiagnosticProvenance::VirtualExpansion {
        source: source_buffer_source,
        range: source_buffer_range,
    })
}

fn expansion_authority_unavailable() -> DiagnosticProvenance {
    DiagnosticProvenance::Unavailable(PreprocUnavailable::Source(
        SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
    ))
}
