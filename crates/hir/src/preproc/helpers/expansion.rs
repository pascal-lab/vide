use super::*;

pub(in crate::preproc) fn map_macro_expansion(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansion,
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
    })
}

fn map_macro_expansion_definition(
    mapped: &MappedSourcePreprocModel,
    expansion: &SourceMacroExpansion,
) -> PreprocResult<(Option<MacroDefinitionId>, MacroExpansionDefinition)> {
    match &expansion.definition {
        SourceMacroExpansionDefinition::Source(definition_id) => {
            let Some(definition) = mapped.model.macro_definitions().get(*definition_id) else {
                return Err(PreprocError::Unavailable {
                    reason: PreprocUnavailable::Source(
                        SourcePreprocUnavailable::MissingMacroExpansion { call: expansion.call },
                    ),
                });
            };
            Ok((
                Some((*definition_id).into()),
                MacroExpansionDefinition::Source(map_macro_definition(mapped, definition)?),
            ))
        }
        SourceMacroExpansionDefinition::Builtin { name } => {
            Ok((None, MacroExpansionDefinition::Builtin { name: name.clone() }))
        }
    }
}

pub(in crate::preproc) fn map_expansion_display_source(
    mapped: &MappedSourcePreprocModel,
    expansion: SourceMacroExpansionId,
) -> PreprocResult<PreprocSourceMapping> {
    mapped.source_map.expansion_display_source(expansion).map_err(PreprocError::SourceMap)
}

pub(in crate::preproc) fn map_expansion_source_buffer(
    mapped: &MappedSourcePreprocModel,
    expansion: SourceMacroExpansionId,
) -> PreprocResult<PreprocSourceMapping> {
    mapped.source_map.expansion_source_buffer(expansion).map_err(PreprocError::SourceMap)
}

pub(in crate::preproc) fn display_only_virtual_expansion_unavailable(
    source: &PreprocSourceMapping,
) -> PreprocUnavailable {
    match source {
        PreprocSourceMapping::VirtualDisplay { path, origin } => {
            PreprocUnavailable::DisplayOnlyVirtualExpansion {
                path: path.clone(),
                origin: origin.clone(),
            }
        }
        PreprocSourceMapping::RealFile(_)
        | PreprocSourceMapping::VirtualFile { .. }
        | PreprocSourceMapping::Unmapped(_) => {
            PreprocUnavailable::Source(SourcePreprocUnavailable::ExpansionAuthorityUnavailable)
        }
    }
}

pub(in crate::preproc) fn source_macro_calls_at(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    offset: TextSize,
) -> Vec<&SourceMacroCall> {
    mapped
        .macro_call_ids_at(file_id, offset)
        .into_iter()
        .filter_map(|call| mapped.model.macro_calls().get(call))
        .collect()
}

pub(in crate::preproc) fn source_macro_calls_intersecting_range(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    source_range: TextRange,
) -> Vec<&SourceMacroCall> {
    mapped
        .macro_call_ids_intersecting_range(file_id, source_range)
        .into_iter()
        .filter_map(|call| mapped.model.macro_calls().get(call))
        .collect()
}

pub(in crate::preproc) fn immediate_macro_expansion_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCall,
) -> PreprocResult<MacroExpansionQuery> {
    let call = map_macro_call(mapped, call_fact)?;
    Ok(match mapped.model.immediate_macro_expansion(call_fact.id) {
        SourceMacroExpansionQuery::Available(expansion) => {
            let Some(expansion) = mapped.model.macro_expansions().get(expansion) else {
                return Ok(MacroExpansionQuery::Unavailable(Box::new(MacroExpansionUnavailable {
                    call,
                    reason: PreprocUnavailable::Source(
                        SourcePreprocUnavailable::MissingMacroExpansion { call: call_fact.id },
                    ),
                })));
            };
            MacroExpansionQuery::Available(Box::new(map_macro_expansion(mapped, expansion)?))
        }
        SourceMacroExpansionQuery::Unavailable(reason) => {
            MacroExpansionQuery::Unavailable(Box::new(MacroExpansionUnavailable {
                call,
                reason: PreprocUnavailable::Source(reason),
            }))
        }
    })
}

pub(in crate::preproc) fn recursive_macro_expansion_for_call(
    mapped: &MappedSourcePreprocModel,
    call_fact: &SourceMacroCall,
) -> PreprocResult<RecursiveMacroExpansion> {
    let root_call = map_macro_call(mapped, call_fact)?;
    let recursive = mapped.model.recursive_macro_expansion(call_fact.id);
    let expansions = recursive
        .expansions
        .into_iter()
        .filter_map(|expansion| mapped.model.macro_expansions().get(expansion))
        .map(|expansion| map_macro_expansion(mapped, expansion))
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

    Ok(RecursiveMacroExpansion { root_call, expansions, unavailable })
}
