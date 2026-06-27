use super::*;

pub(crate) fn map_macro_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinition,
) -> PreprocResult<MacroDefinition> {
    let (mut file_id, mut directive_range, mut name_range) = map_definition_ranges(
        mapped,
        definition.event_id.raw(),
        definition.directive_range,
        definition.name_range,
    )?;
    if let Some(manifest_source) =
        mapped.source_map.predefine_manifest_source(definition.name_range.source)
    {
        file_id = manifest_source.file_id;
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
                        .map(|range| map_source_range(mapped, range).map(|(_, range)| range))
                        .transpose()?;
                    Ok(MacroDefinitionParam { param_index, name: param.name.clone(), range })
                })
                .collect::<PreprocResult<Vec<_>>>()
        })
        .transpose()?;
    let source_range = definition_source_range(mapped, file_id, directive_range, definition);
    Ok(MacroDefinition {
        id: definition.id.into(),
        file_id,
        name: definition.name.clone(),
        params,
        body_tokens: definition.body_tokens.iter().map(|token| token.raw.clone()).collect(),
        source_range,
        directive_range,
        name_range,
    })
}

fn definition_source_range(
    mapped: &MappedSourcePreprocModel,
    file_id: FileId,
    directive_range: TextRange,
    definition: &SourceMacroDefinition,
) -> TextRange {
    let mut source_range = directive_range;
    for token_range in definition.body_tokens.iter().filter_map(|token| token.range) {
        let Ok((token_file_id, token_range)) = map_source_range(mapped, token_range) else {
            continue;
        };
        if token_file_id == file_id && token_range.end() > source_range.end() {
            source_range = TextRange::new(source_range.start(), token_range.end());
        }
    }
    source_range
}

pub(in crate::preproc) fn map_macro_param_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinition,
    param_index: usize,
    param: &SourceMacroParam,
) -> PreprocResult<Option<MacroParamDefinition>> {
    let Some(name) = &param.name else {
        return Ok(None);
    };
    let Some(name_source_range) = param.name_range else {
        return Ok(None);
    };
    let macro_definition = map_macro_definition(mapped, definition)?;
    let (name_file_id, range) = map_source_range(mapped, name_source_range)?;
    if name_file_id != macro_definition.file_id {
        return Err(PreprocError::MismatchedRangeFiles {
            kind: RangeFilesKind::Definition,
            event_id: definition.event_id.raw(),
            directive_file_id: macro_definition.file_id,
            name_file_id,
        });
    }
    let param_range = param
        .range
        .map(|range| map_source_range(mapped, range).map(|(_, range)| range))
        .transpose()?;

    Ok(Some(MacroParamDefinition {
        macro_definition,
        param_index,
        name: name.clone(),
        range,
        param_range,
    }))
}

pub(in crate::preproc) fn map_definition_ranges(
    mapped: &MappedSourcePreprocModel,
    event_id: u32,
    directive_source_range: SourceRange,
    name_source_range: SourceRange,
) -> PreprocResult<(FileId, TextRange, TextRange)> {
    let (directive_file_id, directive_range) = map_source_range(mapped, directive_source_range)?;
    let (name_file_id, name_range) = map_source_range(mapped, name_source_range)?;
    if directive_file_id != name_file_id {
        return Err(PreprocError::MismatchedRangeFiles {
            kind: RangeFilesKind::Definition,
            event_id,
            directive_file_id,
            name_file_id,
        });
    }
    Ok((directive_file_id, directive_range, name_range))
}

pub(in crate::preproc) fn same_macro_definition(
    left: &MacroDefinition,
    right: &MacroDefinition,
) -> bool {
    MacroDefinitionKey::from_definition(left) == MacroDefinitionKey::from_definition(right)
}
