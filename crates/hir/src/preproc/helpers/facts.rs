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
        tokens: argument
            .tokens
            .iter()
            .map(|token| map_macro_argument_token(mapped, token))
            .collect::<PreprocResult<Vec<_>>>()?,
    })
}

fn map_macro_argument_token(
    mapped: &MappedSourcePreprocModel,
    token: &preproc::source::SourceMacroToken,
) -> PreprocResult<MacroArgumentToken> {
    let (source, range) = token
        .range
        .map(|range| map_mapped_source_range(mapped, range))
        .transpose()?
        .map_or((None, None), |(source, range)| (Some(source), Some(range)));
    Ok(MacroArgumentToken { raw: token.raw.clone(), source, range })
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
