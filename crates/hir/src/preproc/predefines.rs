use super::*;

pub(super) fn configured_predefine_names(db: &dyn SourceRootDb, file_id: FileId) -> Vec<SmolStr> {
    let mut names = UniqVec::<SmolStr, SmolStr>::default();

    let profile_id = db.file_compilation_profile(file_id);
    for predefine in &db.project_config().preprocess_for_profile(profile_id).predefines {
        if let Some(name) = predefine_macro_name(predefine.as_str()) {
            names.push_unique(name);
        }
    }

    for predefine in &db.file_preprocess_config(file_id).predefines {
        if let Some(name) = predefine_macro_name(predefine.as_str()) {
            names.push_unique(name);
        }
    }

    names.into_vec()
}

fn predefine_macro_name(predefine: &str) -> Option<SmolStr> {
    let name = predefine.split_once('=').map_or(predefine, |(name, _)| name);
    let name = name.trim().strip_prefix('`').unwrap_or(name.trim());
    if name.is_empty() { None } else { Some(SmolStr::new(name)) }
}

pub(super) fn configured_predefine_definitions_for_name(
    db: &dyn SourceRootDb,
    context_file_id: FileId,
    name: &SmolStr,
) -> Vec<MacroDefinition> {
    let mut definitions = UniqVec::<MacroDefinition, MacroDefinitionKey>::default();
    let profile_id = db.file_compilation_profile(context_file_id);
    let project_preprocess = db.project_config().preprocess_for_profile(profile_id);
    for predefine in &project_preprocess.predefines {
        if let Some(definition) = configured_predefine_definition(db, predefine, name) {
            definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
        }
    }
    for predefine in &db.file_preprocess_config(context_file_id).predefines {
        if let Some(definition) = configured_predefine_definition(db, predefine, name) {
            definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
        }
    }
    definitions.into_vec()
}

pub(super) fn configured_predefine_definitions_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroDefinition>> {
    let mut definitions = UniqVec::<MacroDefinition, MacroDefinitionKey>::default();
    let contexts = source_preproc_single_query_contexts(db, file_id);
    for context_file_id in contexts.model_file_ids.iter().copied() {
        let profile_id = db.file_compilation_profile(context_file_id);
        let project_preprocess = db.project_config().preprocess_for_profile(profile_id);
        for predefine in &project_preprocess.predefines {
            if let Some(definition) =
                configured_predefine_definition_at(db, predefine, file_id, offset)
            {
                definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
            }
        }
        for predefine in &db.file_preprocess_config(context_file_id).predefines {
            if let Some(definition) =
                configured_predefine_definition_at(db, predefine, file_id, offset)
            {
                definitions.push_keyed(definition, MacroDefinitionKey::from_definition);
            }
        }
    }
    if definitions.is_empty() {
        finish_empty_single_query(&contexts, None)?;
    }
    Ok(definitions.into_vec())
}

fn configured_predefine_definition_at(
    db: &dyn SourceRootDb,
    predefine: &Predefine,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroDefinition> {
    let definition =
        configured_predefine_definition(db, predefine, &predefine_macro_name(predefine.as_str())?)?;
    (definition.file_id == file_id && definition.name_range.contains(offset)).then_some(definition)
}

fn configured_predefine_definition(
    db: &dyn SourceRootDb,
    predefine: &Predefine,
    name: &SmolStr,
) -> Option<MacroDefinition> {
    let predefine_name = predefine_macro_name(predefine.as_str())?;
    if &predefine_name != name {
        return None;
    }
    let source = predefine.source.as_ref()?;
    let file_id = file_id_for_predefine_source_path(db, &source.path)?;
    Some(MacroDefinition {
        id: MacroDefinitionId::ConfiguredPredefine { file_id, range: source.range },
        file_id,
        name: predefine_name,
        params: None,
        body_tokens: Vec::new(),
        define_index: CONFIGURED_PREDEFINE_DEFINE_INDEX,
        event_id: CONFIGURED_PREDEFINE_EVENT_ID,
        directive_range: source.range,
        name_range: source.range,
    })
}

fn file_id_for_predefine_source_path(
    db: &dyn SourceRootDb,
    path: &utils::paths::AbsPathBuf,
) -> Option<FileId> {
    db.files().iter().copied().find(|file_id| db.file_path(*file_id).as_ref() == Some(path))
}
