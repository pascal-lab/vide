use super::*;

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
