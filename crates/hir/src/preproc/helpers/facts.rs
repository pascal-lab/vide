use super::*;

pub(in crate::preproc) fn map_macro_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
) -> PreprocResult<MacroDefinition> {
    let (mut source, mut directive_range, mut name_range) = map_definition_ranges(
        mapped,
        definition.event_id.raw(),
        definition.directive_range,
        definition.name_range,
    )?;
    if let Some(manifest_source) =
        mapped.source_map.predefine_manifest_source(definition.name_range.source)
    {
        source = MappedPreprocSource::RealFile { file_id: manifest_source.file_id };
        directive_range = manifest_source.range;
        name_range = manifest_source.range;
    }
    let params = definition
        .params
        .as_ref()
        .map(|params| {
            params
                .iter()
                .enumerate()
                .map(|(param_index, param)| {
                    let range = param
                        .name_range
                        .map(|range| map_mapped_source_range(mapped, range).map(|(_, range)| range))
                        .transpose()?;
                    Ok(MacroDefinitionParam { param_index, name: param.name.clone(), range })
                })
                .collect::<PreprocResult<Vec<_>>>()
        })
        .transpose()?;
    let file_id = require_file_backed_source(&source)?;
    Ok(MacroDefinition {
        id: definition.id.into(),
        file_id,
        source,
        capability: capability_status(&mapped.model.capabilities().definition_name_ranges),
        name: definition.name.clone(),
        params,
        body_tokens: definition.body_tokens.iter().map(|token| token.raw.clone()).collect(),
        define_index: define_index_for_definition(mapped, definition)?,
        event_id: definition.event_id.raw(),
        directive_range,
        name_range,
    })
}

pub(in crate::preproc) fn map_macro_param_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
    param_index: usize,
    param: &SourceMacroParamFact,
) -> PreprocResult<Option<MacroParamDefinition>> {
    let Some(name) = &param.name else {
        return Ok(None);
    };
    let Some(name_source_range) = param.name_range else {
        return Ok(None);
    };
    let macro_definition = map_macro_definition(mapped, definition)?;
    let (source, range) = map_mapped_source_range(mapped, name_source_range)?;
    let name_file_id = require_file_backed_source(&source)?;
    if name_file_id != macro_definition.file_id {
        return Err(PreprocError::MismatchedDefinitionRangeFiles {
            event_id: definition.event_id.raw(),
            directive_file_id: macro_definition.file_id,
            name_file_id,
        });
    }
    let param_range = param
        .range
        .map(|range| map_mapped_source_range(mapped, range).map(|(_, range)| range))
        .transpose()?;

    Ok(Some(MacroParamDefinition {
        macro_definition,
        param_index,
        name: name.clone(),
        range,
        param_range,
    }))
}

pub(in crate::preproc) fn map_macro_param_reference(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
    param_index: usize,
    token_index: usize,
    token_range: SourceRange,
) -> PreprocResult<MacroParamReference> {
    let macro_definition = map_macro_definition(mapped, definition)?;
    let (source, range) = map_mapped_source_range(mapped, token_range)?;
    let file_id = require_file_backed_source(&source)?;
    let name = definition
        .params
        .as_ref()
        .and_then(|params| params.get(param_index))
        .and_then(|param| param.name.clone())
        .ok_or_else(|| {
            PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() },
            ))
        })?;

    Ok(MacroParamReference {
        macro_definition,
        source,
        capability: PreprocAvailability::Complete,
        file_id,
        param_index,
        token_index,
        name,
        range,
    })
}

pub(in crate::preproc) fn map_definition_provenance_from_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
) -> PreprocResult<MacroDefinitionProvenance> {
    let definition = map_macro_definition(mapped, definition)?;
    Ok(MacroDefinitionProvenance {
        id: definition.id,
        source: definition.source,
        capability: definition.capability,
        event_id: definition.event_id,
        file_id: definition.file_id,
        directive_range: definition.directive_range,
        name_range: definition.name_range,
    })
}

pub(in crate::preproc) fn map_macro_reference(
    mapped: &MappedSourcePreprocModel,
    reference: &SourceMacroReferenceFact,
) -> PreprocResult<MacroReference> {
    let (source, directive_range, name_range) = map_reference_ranges(mapped, reference)?;
    let file_id = require_file_backed_source(&source)?;
    Ok(MacroReference {
        id: reference.id.into(),
        file_id,
        source,
        capability: capability_status(&mapped.model.capabilities().macro_reference_resolution),
        name: reference.name.clone(),
        directive_range,
        range: name_range,
        resolution: map_macro_resolution(mapped, &reference.resolution)?,
    })
}

pub(in crate::preproc) fn map_macro_call(
    mapped: &MappedSourcePreprocModel,
    call: &SourceMacroCallFact,
) -> PreprocResult<MacroCall> {
    let (source, range) = map_mapped_source_range(mapped, call.call_range)?;
    let arguments = call
        .arguments
        .iter()
        .map(|argument| map_macro_argument(mapped, argument))
        .collect::<PreprocResult<Vec<_>>>()?;
    let file_id = require_file_backed_source(&source)?;
    Ok(MacroCall {
        id: call.id.into(),
        reference_id: call.reference.into(),
        file_id,
        source,
        capability: macro_call_availability(&call.status),
        arguments,
        directive_range: range,
        range,
        callee: map_macro_resolution(mapped, &call.callee)?,
        expansion: call.expansion.map(Into::into),
    })
}

pub(in crate::preproc) fn map_macro_argument(
    mapped: &MappedSourcePreprocModel,
    argument: &SourceMacroArgumentFact,
) -> PreprocResult<MacroArgument> {
    let (source, range) = argument
        .argument_range
        .map(|range| map_mapped_source_range(mapped, range))
        .transpose()?
        .map_or((None, None), |(source, range)| (Some(source), Some(range)));
    Ok(MacroArgument {
        argument_index: argument.argument_index,
        source,
        range,
        tokens: argument.tokens.iter().map(|token| token.raw.clone()).collect(),
    })
}

pub(in crate::preproc) fn map_macro_resolution(
    mapped: &MappedSourcePreprocModel,
    resolution: &SourceMacroResolutionFact,
) -> PreprocResult<MacroResolution> {
    Ok(match resolution {
        SourceMacroResolutionFact::Resolved { definition, reason, include_chain } => {
            MacroResolution::Resolved {
                definition_id: (*definition).into(),
                reason: map_macro_resolution_reason(*reason),
                include_chain: map_include_chain(mapped, include_chain)?,
            }
        }
        SourceMacroResolutionFact::Undefined => MacroResolution::Undefined,
        SourceMacroResolutionFact::Unavailable(reason) => {
            MacroResolution::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    })
}

pub(in crate::preproc) fn map_macro_resolution_reason(
    reason: SourceMacroResolutionReasonFact,
) -> MacroResolutionReason {
    match reason {
        SourceMacroResolutionReasonFact::VisibleDefinition => {
            MacroResolutionReason::VisibleDefinition
        }
        SourceMacroResolutionReasonFact::IncludeGuardIfNDef => {
            MacroResolutionReason::IncludeGuardIfNDef
        }
    }
}

pub(in crate::preproc) fn map_reference_ranges(
    mapped: &MappedSourcePreprocModel,
    reference: &SourceMacroReferenceFact,
) -> PreprocResult<(MappedPreprocSource, TextRange, TextRange)> {
    let (directive_source, directive_range) =
        map_mapped_source_range(mapped, reference.directive_range)?;
    let (name_source, name_range) = map_mapped_source_range(mapped, reference.name_range)?;
    if directive_source != name_source {
        let directive_file_id = require_file_backed_source(&directive_source)?;
        let name_file_id = require_file_backed_source(&name_source)?;
        return Err(PreprocError::MismatchedReferenceRangeFiles {
            event_id: reference.event_id.raw(),
            directive_file_id,
            name_file_id,
        });
    }
    Ok((directive_source, directive_range, name_range))
}

pub(in crate::preproc) fn map_include_status(
    mapped: &MappedSourcePreprocModel,
    status: &SourceIncludeStatus,
) -> PreprocResult<IncludeDirectiveStatus> {
    Ok(match status {
        SourceIncludeStatus::Resolved { source } => {
            IncludeDirectiveStatus::Resolved { source: map_mapped_source_id(mapped, *source)? }
        }
        SourceIncludeStatus::Unresolved => IncludeDirectiveStatus::Unresolved,
        SourceIncludeStatus::Unavailable(reason) => {
            IncludeDirectiveStatus::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    })
}

pub(in crate::preproc) fn capability_status(status: &CapabilityStatus) -> PreprocAvailability {
    match status {
        CapabilityStatus::Complete => PreprocAvailability::Complete,
        CapabilityStatus::Partial => PreprocAvailability::Partial,
        CapabilityStatus::Unavailable(reason) => {
            PreprocAvailability::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    }
}

pub(in crate::preproc) fn macro_call_availability(
    status: &SourceMacroCallStatusFact,
) -> PreprocAvailability {
    match status {
        SourceMacroCallStatusFact::ExpansionAvailable => PreprocAvailability::Complete,
        SourceMacroCallStatusFact::ExpansionUnavailable(reason) => {
            PreprocAvailability::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    }
}

pub(in crate::preproc) fn macro_expansion_availability(
    status: &SourceMacroExpansionStatusFact,
) -> PreprocAvailability {
    match status {
        SourceMacroExpansionStatusFact::Complete => PreprocAvailability::Complete,
        SourceMacroExpansionStatusFact::Unavailable(reason) => {
            PreprocAvailability::Unavailable(PreprocUnavailable::Source(reason.clone()))
        }
    }
}

pub(in crate::preproc) fn unavailable_error(reason: SourcePreprocUnavailable) -> PreprocError {
    PreprocError::Unavailable { reason: PreprocUnavailable::Source(reason) }
}

pub(in crate::preproc) fn define_index_for_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinitionFact,
) -> PreprocResult<usize> {
    mapped
        .model
        .defines()
        .iter()
        .position(|define| define.event_id == definition.event_id)
        .ok_or_else(|| {
            PreprocError::SourceQuery(SourcePreprocQueryError::Model(
                SourcePreprocError::MissingEvent { event_id: definition.event_id.raw() },
            ))
        })
}

pub(in crate::preproc) fn map_definition_ranges(
    mapped: &MappedSourcePreprocModel,
    event_id: u32,
    directive_source_range: SourceRange,
    name_source_range: SourceRange,
) -> PreprocResult<(MappedPreprocSource, TextRange, TextRange)> {
    let (directive_source, directive_range) =
        map_mapped_source_range(mapped, directive_source_range)?;
    let (name_source, name_range) = map_mapped_source_range(mapped, name_source_range)?;
    if directive_source != name_source {
        let directive_file_id = require_file_backed_source(&directive_source)?;
        let name_file_id = require_file_backed_source(&name_source)?;
        return Err(PreprocError::MismatchedDefinitionRangeFiles {
            event_id,
            directive_file_id,
            name_file_id,
        });
    }
    Ok((directive_source, directive_range, name_range))
}

pub(in crate::preproc) fn map_include_chain(
    mapped: &MappedSourcePreprocModel,
    chain: &[SourceIncludeChainEntry],
) -> PreprocResult<Vec<IncludeChainEntry>> {
    chain
        .iter()
        .map(|entry| {
            let (include_file_id, include_range) = map_source_range(mapped, entry.include_range)?;
            let included_file_id = map_source_id(mapped, entry.included_source)?;
            Ok(IncludeChainEntry {
                include_event_id: entry.include_event_id.raw(),
                include_file_id,
                include_range,
                included_file_id,
            })
        })
        .collect()
}

pub(in crate::preproc) fn macro_reference_context_capability(
    references: &[MacroReference],
) -> PreprocAvailability {
    if references
        .iter()
        .all(|reference| matches!(reference.capability, PreprocAvailability::Complete))
    {
        return PreprocAvailability::Complete;
    }
    if references
        .iter()
        .any(|reference| matches!(reference.capability, PreprocAvailability::Partial))
    {
        return PreprocAvailability::Partial;
    }
    references
        .iter()
        .find_map(|reference| match &reference.capability {
            PreprocAvailability::Unavailable(reason) => {
                Some(PreprocAvailability::Unavailable(reason.clone()))
            }
            PreprocAvailability::Complete | PreprocAvailability::Partial => None,
        })
        .unwrap_or(PreprocAvailability::Complete)
}

pub(in crate::preproc) fn same_macro_definition(
    left: &MacroDefinition,
    right: &MacroDefinition,
) -> bool {
    MacroDefinitionKey::from_definition(left) == MacroDefinitionKey::from_definition(right)
}

pub(in crate::preproc) fn macro_param_reference_context_capability(
    references: &[MacroParamReference],
) -> PreprocAvailability {
    if references
        .iter()
        .any(|reference| matches!(reference.capability, PreprocAvailability::Partial))
    {
        return PreprocAvailability::Partial;
    }
    references
        .iter()
        .find_map(|reference| match &reference.capability {
            PreprocAvailability::Unavailable(reason) => {
                Some(PreprocAvailability::Unavailable(reason.clone()))
            }
            PreprocAvailability::Complete | PreprocAvailability::Partial => None,
        })
        .unwrap_or(PreprocAvailability::Complete)
}
