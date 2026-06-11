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
