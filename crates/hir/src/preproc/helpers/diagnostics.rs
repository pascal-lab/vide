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

pub(in crate::preproc) fn map_token_provenance(
    mapped: &MappedSourcePreprocModel,
    provenance: &SourceTokenProvenanceFact,
) -> PreprocResult<TokenProvenance> {
    Ok(match provenance {
        SourceTokenProvenanceFact::Source { token_range } => {
            let (source, range) = map_mapped_source_range(mapped, *token_range)?;
            TokenProvenance::SourceToken { source, range }
        }
        SourceTokenProvenanceFact::MacroBody { definition, body_token_range, call, .. } => {
            let call = mapped_macro_call(mapped, *call)?;
            let (source, range) = map_mapped_source_range(mapped, *body_token_range)?;
            TokenProvenance::MacroBody { call, definition_id: (*definition).into(), source, range }
        }
        SourceTokenProvenanceFact::MacroArgument {
            call,
            argument_index,
            argument_token_range,
            ..
        } => {
            let call = mapped_macro_call(mapped, *call)?;
            let Ok((source, range)) = map_mapped_source_range(mapped, *argument_token_range) else {
                return Ok(TokenProvenance::Unavailable);
            };
            TokenProvenance::MacroArgument { call, argument_index: *argument_index, source, range }
        }
        SourceTokenProvenanceFact::TokenPaste { call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            TokenProvenance::TokenPaste
        }
        SourceTokenProvenanceFact::Stringification { call, .. } => {
            let _call = mapped_macro_call(mapped, *call)?;
            TokenProvenance::Stringification
        }
        SourceTokenProvenanceFact::Predefine { source } => {
            let _source = map_mapped_source_id(mapped, *source)?;
            TokenProvenance::Predefine
        }
        SourceTokenProvenanceFact::Builtin { name, call, .. } => {
            TokenProvenance::Builtin { name: name.clone(), call: mapped_macro_call(mapped, *call)? }
        }
        SourceTokenProvenanceFact::Unavailable(_) => TokenProvenance::Unavailable,
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
        match map_token_provenance(mapped, provenance)? {
            TokenProvenance::SourceToken { source, range } => {
                return Ok(DiagnosticProvenance::SourceToken { source, range });
            }
            TokenProvenance::MacroBody { call, definition_id, source, range, .. } => {
                return Ok(DiagnosticProvenance::MacroBody { call, definition_id, source, range });
            }
            TokenProvenance::MacroArgument { call, argument_index, source, range, .. } => {
                return Ok(DiagnosticProvenance::MacroArgument {
                    call,
                    argument_index,
                    source,
                    range,
                });
            }
            TokenProvenance::Unavailable => {
                saw_unavailable = Some(PreprocUnavailable::Source(
                    SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
                ));
            }
            TokenProvenance::TokenPaste | TokenProvenance::Stringification => {
                saw_unavailable = Some(PreprocUnavailable::Source(
                    SourcePreprocUnavailable::ExpansionAuthorityUnavailable,
                ));
            }
            TokenProvenance::Predefine => {}
            TokenProvenance::Builtin { call, name } => {
                return Ok(DiagnosticProvenance::Builtin {
                    call: call.clone(),
                    name: name.clone(),
                });
            }
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
