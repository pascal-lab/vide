use super::*;

pub(in crate::preproc) fn map_macro_expansion(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansionFact,
) -> PreprocResult<MacroExpansion> {
    let Some(call) = mapped.model.macro_calls().get(expansion.call) else {
        return Err(PreprocError::Unavailable {
            reason: PreprocUnavailable::Source(SourcePreprocUnavailable::MissingMacroCall {
                call: expansion.call,
            }),
        });
    };
    let (definition_id, definition) = map_macro_expansion_definition(mapped, expansion)?;
    Ok(MacroExpansion {
        id: expansion.id.into(),
        call: map_macro_call(mapped, call)?,
        definition_id,
        definition,
        emitted_token_range: expansion.emitted_token_range,
        display_text: mapped
            .source_map
            .expansion_display_text(expansion.id)
            .ok_or(PreprocError::SourceMap(PreprocSourceMapError::MissingExpansionVirtualFile {
                expansion: expansion.id,
            }))?
            .to_owned(),
        display_source: map_expansion_display_source(mapped, expansion.id)?,
        display_range: mapped
            .source_map
            .emitted_display_range(expansion.id, expansion.emitted_token_range)
            .map_err(PreprocError::SourceMap)?,
        child_calls: expansion.child_calls.iter().copied().map(Into::into).collect(),
        capability: macro_expansion_availability(&expansion.status),
    })
}

fn map_macro_expansion_definition(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansionFact,
) -> PreprocResult<(Option<MacroDefinitionId>, MacroExpansionDefinition)> {
    match &expansion.definition {
        SourceMacroExpansionDefinitionFact::Source(definition_id) => {
            let Some(definition) = mapped.model.macro_definitions().get(*definition_id) else {
                return Err(PreprocError::Unavailable {
                    reason: PreprocUnavailable::Source(
                        SourcePreprocUnavailable::MissingEmittedTokenMacroDefinition {
                            call: expansion.call,
                        },
                    ),
                });
            };
            Ok((
                Some((*definition_id).into()),
                MacroExpansionDefinition::Source(map_macro_definition(mapped, definition)?),
            ))
        }
        SourceMacroExpansionDefinitionFact::Builtin { name } => Ok((
            None,
            MacroExpansionDefinition::Builtin {
                name: name.clone(),
                capability: macro_expansion_availability(&expansion.status),
            },
        )),
    }
}

pub(in crate::preproc) fn map_expansion_display_source(
    mapped: &MappedSourcePreprocModel,
    expansion: SourceMacroExpansionId,
) -> PreprocResult<MappedPreprocSource> {
    match mapped.source_map.expansion_display_source(expansion).map_err(PreprocError::SourceMap)? {
        PreprocSourceMapping::VirtualFile { file_id, path, origin } => {
            Ok(MappedPreprocSource::VirtualFile { file_id, path, origin })
        }
        PreprocSourceMapping::VirtualDisplay { path, origin } => {
            Ok(MappedPreprocSource::VirtualDisplay { path, origin })
        }
        PreprocSourceMapping::RealFile(file_id) => Ok(MappedPreprocSource::RealFile { file_id }),
        PreprocSourceMapping::Unmapped(reason) => {
            Err(PreprocError::Unavailable { reason: PreprocUnavailable::Source(reason) })
        }
    }
}

pub(in crate::preproc) fn source_macro_calls_intersecting_range(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    source_range: TextRange,
) -> Vec<&SourceMacroCallFact> {
    mapped
        .macro_call_ids_intersecting_range(file_id, source_range)
        .into_iter()
        .filter_map(|call| mapped.model.macro_calls().get(call))
        .collect()
}

pub(in crate::preproc) fn recursive_macro_expansion_provenance_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCallFact,
) -> PreprocResult<RecursiveMacroExpansionProvenance> {
    let root_call = map_macro_call(mapped, call_fact)?;
    let recursive = mapped.model.recursive_macro_expansion(call_fact.id);
    let expansions = recursive
        .expansions
        .into_iter()
        .filter_map(|expansion| mapped.model.macro_expansions().get(expansion))
        .map(|expansion| macro_expansion_provenance_for_expansion(mapped, expansion))
        .collect::<PreprocResult<Vec<_>>>()?;
    let unavailable = recursive
        .unavailable
        .into_iter()
        .map(|unavailable| {
            let Some(call) = mapped.model.macro_calls().get(unavailable.call) else {
                return Err(PreprocError::Unavailable {
                    reason: PreprocUnavailable::Source(
                        SourcePreprocUnavailable::MissingMacroCall { call: unavailable.call },
                    ),
                });
            };
            Ok(MacroExpansionUnavailable {
                call: map_macro_call(mapped, call)?,
                reason: PreprocUnavailable::Source(unavailable.reason),
            })
        })
        .collect::<PreprocResult<Vec<_>>>()?;

    Ok(RecursiveMacroExpansionProvenance { root_call, expansions, unavailable })
}

pub(in crate::preproc) fn macro_expansion_provenance_for_expansion(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansionFact,
) -> PreprocResult<MacroExpansionProvenance> {
    let expansion_id = expansion.id;
    let expansion = map_macro_expansion(mapped, expansion)?;
    let mut tokens = Vec::new();
    for token_id in emitted_token_ids(expansion.emitted_token_range) {
        let Some(token) = mapped.model.emitted_tokens().get(token_id) else {
            return Err(PreprocError::SourceMap(PreprocSourceMapError::MissingEmittedToken {
                token: token_id,
            }));
        };
        let Some(provenance) = mapped.model.token_provenance().get(token.provenance) else {
            return Err(unavailable_error(
                SourcePreprocUnavailable::TokenProvenanceAuthorityUnavailable,
            ));
        };
        tokens.push(EmittedTokenProvenance {
            token: token_id,
            text: token.text.clone(),
            display_range: mapped
                .source_map
                .emitted_token_display_range(expansion_id, token_id)
                .map_err(PreprocError::SourceMap)?,
            provenance: map_token_provenance(mapped, provenance)?,
        });
    }

    Ok(MacroExpansionProvenance { expansion, tokens })
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
        SourceTokenProvenanceFact::MacroBody {
            definition,
            body_token_range,
            call,
            identity,
            ..
        } => {
            let call = mapped_macro_call(mapped, *call)?;
            let (source, range) = map_mapped_source_range(mapped, *body_token_range)?;
            TokenProvenance::MacroBody {
                identity: (*identity).into(),
                call,
                definition_id: (*definition).into(),
                source,
                range,
            }
        }
        SourceTokenProvenanceFact::MacroArgument {
            call,
            argument_index,
            argument_token_range,
            identity,
            ..
        } => {
            let call = mapped_macro_call(mapped, *call)?;
            let Ok((source, range)) = map_mapped_source_range(mapped, *argument_token_range) else {
                return Ok(TokenProvenance::Unavailable(PreprocUnavailable::Source(
                    SourcePreprocUnavailable::UnsupportedEmittedTokenProvenance,
                )));
            };
            TokenProvenance::MacroArgument {
                identity: (*identity).into(),
                call,
                argument_index: *argument_index,
                source,
                range,
            }
        }
        SourceTokenProvenanceFact::TokenPaste { call, identity, .. } => {
            TokenProvenance::TokenPaste {
                identity: (*identity).into(),
                call: mapped_macro_call(mapped, *call)?,
            }
        }
        SourceTokenProvenanceFact::Stringification { call, identity, .. } => {
            TokenProvenance::Stringification {
                identity: (*identity).into(),
                call: mapped_macro_call(mapped, *call)?,
            }
        }
        SourceTokenProvenanceFact::Predefine { source } => {
            TokenProvenance::Predefine { source: map_mapped_source_id(mapped, *source)? }
        }
        SourceTokenProvenanceFact::Builtin { name, call, .. } => {
            TokenProvenance::Builtin { name: name.clone(), call: mapped_macro_call(mapped, *call)? }
        }
        SourceTokenProvenanceFact::Unavailable(reason) => {
            TokenProvenance::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
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
