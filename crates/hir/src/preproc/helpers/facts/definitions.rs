use super::*;

pub(in crate::preproc) fn map_macro_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinition,
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

pub(in crate::preproc) fn define_index_for_definition(
    mapped: &MappedSourcePreprocModel,
    definition: &SourceMacroDefinition,
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

pub(in crate::preproc) fn same_macro_definition(
    left: &MacroDefinition,
    right: &MacroDefinition,
) -> bool {
    MacroDefinitionKey::from_definition(left) == MacroDefinitionKey::from_definition(right)
}
