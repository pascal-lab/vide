use super::*;

pub(in crate::preproc) fn map_macro_param_reference(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinition,
    param_index: usize,
    token_index: usize,
    token_range: SourceRange,
) -> PreprocResult<MacroParamReference> {
    let macro_definition = map_macro_definition(mapped, definition)?;
    let (file_id, range) = map_source_range(mapped, token_range)?;
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

    Ok(MacroParamReference { macro_definition, file_id, param_index, token_index, name, range })
}

pub(in crate::preproc) fn map_macro_reference(
    mapped: &MappedSourcePreprocModel,
    reference: &SourceMacroReference,
) -> PreprocResult<MacroReference> {
    let (file_id, name_range) = map_reference_ranges(mapped, reference)?;
    Ok(MacroReference { file_id, name: reference.name.clone(), range: name_range })
}

pub(in crate::preproc) fn map_macro_call(
    mapped: &MappedSourcePreprocModel,
    call: &SourceMacroCall,
) -> PreprocResult<MacroCall> {
    let (file_id, range) = map_source_range(mapped, call.call_range)?;
    let arguments = call
        .arguments
        .iter()
        .map(|argument| map_macro_argument(mapped, argument))
        .collect::<PreprocResult<Vec<_>>>()?;
    Ok(MacroCall { file_id, arguments, range })
}

pub(in crate::preproc) fn map_macro_argument(
    mapped: &MappedSourcePreprocModel,
    argument: &SourceMacroArgument,
) -> PreprocResult<MacroArgument> {
    let range = argument
        .argument_range
        .map(|range| map_source_range(mapped, range).map(|(_, range)| range))
        .transpose()?;
    Ok(MacroArgument { argument_index: argument.argument_index, range })
}

pub(in crate::preproc) fn map_reference_ranges(
    mapped: &MappedSourcePreprocModel,
    reference: &SourceMacroReference,
) -> PreprocResult<(FileId, TextRange)> {
    let (directive_file_id, _) = map_source_range(mapped, reference.directive_range)?;
    let (name_file_id, name_range) = map_source_range(mapped, reference.name_range)?;
    if directive_file_id != name_file_id {
        return Err(PreprocError::MismatchedRangeFiles {
            kind: RangeFilesKind::Reference,
            event_id: reference.event_id.raw(),
            directive_file_id,
            name_file_id,
        });
    }
    Ok((directive_file_id, name_range))
}
