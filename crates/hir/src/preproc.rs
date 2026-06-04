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
pub struct IncludeDirective {
    pub file_id: FileId,
    pub include_index: usize,
    pub range: TextRange,
    pub target: IncludeTarget,
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

fn resolve_literal_include(db: &dyn SourceRootDb, file_id: FileId, path: &str) -> Option<FileId> {
    let includer_path = db.file_path(file_id)?;
    let include_dirs = db.file_preprocess_config(file_id).include_dirs.clone();
    let path_file_ids = path_file_ids(db);
    resolve_include_target(path, &includer_path, &include_dirs, &path_file_ids)
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
