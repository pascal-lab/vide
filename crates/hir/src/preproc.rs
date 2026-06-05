use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    path_identity::PathIdentityIndex,
    paths::{AbsPathBuf, Utf8Path},
};
use vfs::FileId;

use crate::base_db::source_db::{SourceDb, SourceRootDb};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinition {
    pub file_id: FileId,
    pub name: SmolStr,
    pub define_index: usize,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsage {
    pub file_id: FileId,
    pub name: SmolStr,
    pub usage_index: usize,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsageResolution {
    pub usage: MacroUsage,
    pub definition: MacroDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReference {
    pub file_id: FileId,
    pub name: SmolStr,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroReferenceResolution {
    pub reference: MacroReference,
    pub definition: MacroDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeDirective {
    pub file_id: FileId,
    pub include_index: usize,
    pub range: TextRange,
    pub target: IncludeTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InactiveBranch {
    pub file_id: FileId,
    pub range: TextRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeTarget {
    Literal { path: SmolStr, resolved_file: Option<FileId> },
    Token { raw: SmolStr },
}

pub fn visible_macros_at(
    db: &dyn SourceDb,
    file_id: FileId,
    offset: TextSize,
) -> Vec<MacroDefinition> {
    db.preproc_model(file_id)
        .visible_macros_at(offset)
        .into_iter()
        .filter_map(|binding| {
            let range = binding.define.range?;
            Some(MacroDefinition {
                file_id,
                name: binding.name,
                define_index: binding.define_index,
                range,
            })
        })
        .collect()
}

pub fn macro_definition_at(
    db: &dyn SourceDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroDefinition> {
    let model = db.preproc_model(file_id);
    let (define_index, define) = model.defines().iter().enumerate().find(|(_, define)| {
        define.range.is_some_and(|range| range_contains_offset(range, offset))
    })?;
    Some(MacroDefinition {
        file_id,
        name: define.name.clone()?,
        define_index,
        range: define.range?,
    })
}

pub fn macro_usage_resolution_at(
    db: &dyn SourceDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroUsageResolution> {
    let model = db.preproc_model(file_id);
    let (usage_index, usage) =
        model.usages().iter().enumerate().find(|(_, usage)| {
            usage.range.is_some_and(|range| range_contains_offset(range, offset))
        })?;
    let usage = MacroUsage { file_id, name: usage.name.clone()?, usage_index, range: usage.range? };
    let binding = model.definition_for_usage(usage_index)?;
    let definition = MacroDefinition {
        file_id,
        name: binding.name,
        define_index: binding.define_index,
        range: binding.define.range?,
    };
    Some(MacroUsageResolution { usage, definition })
}

pub fn macro_reference_resolution_at(
    db: &dyn SourceDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<MacroReferenceResolution> {
    if let Some(resolution) = macro_usage_resolution_at(db, file_id, offset) {
        return Some(MacroReferenceResolution {
            reference: MacroReference {
                file_id,
                name: resolution.usage.name,
                range: resolution.usage.range,
            },
            definition: resolution.definition,
        });
    }

    let model = db.preproc_model(file_id);
    for conditional in model.conditionals() {
        for token in &conditional.expr {
            let range = token.range?;
            if !range_contains_offset(range, offset) {
                continue;
            }
            let definition =
                macro_definition_for_name_at(db, file_id, token.value.as_str(), range.start())?;
            return Some(MacroReferenceResolution {
                reference: MacroReference { file_id, name: token.value.clone(), range },
                definition,
            });
        }
    }

    None
}

pub fn macro_references(
    db: &dyn SourceDb,
    file_id: FileId,
    definition: &MacroDefinition,
) -> Vec<MacroReference> {
    if definition.file_id != file_id {
        return Vec::new();
    }

    let model = db.preproc_model(file_id);
    let mut refs = model
        .usages()
        .iter()
        .enumerate()
        .filter_map(|(usage_index, usage)| {
            let binding = model.definition_for_usage(usage_index)?;
            if binding.define_index != definition.define_index {
                return None;
            }
            Some(MacroReference { file_id, name: usage.name.clone()?, range: usage.range? })
        })
        .collect::<Vec<_>>();

    refs.extend(model.conditionals().iter().flat_map(|conditional| {
        conditional.expr.iter().filter_map(|token| {
            let range = token.range?;
            let resolved =
                macro_definition_for_name_at(db, file_id, token.value.as_str(), range.start())?;
            (resolved.define_index == definition.define_index).then(|| MacroReference {
                file_id,
                name: token.value.clone(),
                range,
            })
        })
    }));

    refs
}

pub fn include_directive_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> Option<IncludeDirective> {
    let model = db.preproc_model(file_id);
    let (include_index, include) = model.includes().iter().enumerate().find(|(_, include)| {
        include.range.is_some_and(|range| range_contains_offset(range, offset))
    })?;
    let range = include.range?;
    let target = match &include.target {
        ::preproc::index::MacroIncludeTarget::Literal { path, .. } => IncludeTarget::Literal {
            path: path.clone(),
            resolved_file: resolve_literal_include(db, file_id, path),
        },
        ::preproc::index::MacroIncludeTarget::Token { raw } => {
            IncludeTarget::Token { raw: raw.clone() }
        }
    };
    Some(IncludeDirective { file_id, include_index, range, target })
}

pub fn inactive_branches(db: &dyn SourceDb, file_id: FileId) -> Vec<InactiveBranch> {
    db.preproc_model(file_id)
        .inactive_ranges()
        .iter()
        .copied()
        .map(|range| InactiveBranch { file_id, range })
        .collect()
}

fn resolve_literal_include(db: &dyn SourceRootDb, file_id: FileId, path: &str) -> Option<FileId> {
    let includer_path = db.file_path(file_id)?;
    let include_dirs = db.file_preprocess_config(file_id).include_dirs.clone();
    let path_file_ids = path_file_ids(db);
    resolve_include_target(path, &includer_path, &include_dirs, &path_file_ids)
}

fn macro_definition_for_name_at(
    db: &dyn SourceDb,
    file_id: FileId,
    name: &str,
    offset: TextSize,
) -> Option<MacroDefinition> {
    db.preproc_model(file_id)
        .visible_macros_at(offset)
        .into_iter()
        .find(|binding| binding.name.as_str() == name)
        .and_then(|binding| {
            Some(MacroDefinition {
                file_id,
                name: binding.name,
                define_index: binding.define_index,
                range: binding.define.range?,
            })
        })
}

fn path_file_ids(db: &dyn SourceRootDb) -> PathIdentityIndex<FileId> {
    let mut index = PathIdentityIndex::default();
    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }
        if let Some(path) = db.file_path(file_id) {
            index.insert_path(&path, file_id);
        }
    }
    index
}

fn resolve_include_target(
    path: &str,
    includer_path: &AbsPathBuf,
    include_dirs: &[AbsPathBuf],
    path_file_ids: &PathIdentityIndex<FileId>,
) -> Option<FileId> {
    let include_path = Utf8Path::new(path);
    if include_path.is_absolute() {
        let abs_path = AbsPathBuf::try_from(include_path.to_path_buf()).ok()?.normalize();
        return path_file_ids.get_path(abs_path.as_path());
    }

    if let Some(parent) = includer_path.parent() {
        let candidate = parent.absolutize(include_path);
        if let Some(file_id) = path_file_ids.get_path(candidate.as_path()) {
            return Some(file_id);
        }
    }

    for include_dir in include_dirs {
        let candidate = include_dir.absolutize(include_path);
        if let Some(file_id) = path_file_ids.get_path(candidate.as_path()) {
            return Some(file_id);
        }
    }

    None
}

fn range_contains_offset(range: TextRange, offset: TextSize) -> bool {
    range.start() <= offset && offset <= range.end()
}
