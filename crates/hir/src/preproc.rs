use preproc::source::{SourceMacroBinding, SourcePosition, SourceRange};
use smol_str::SmolStr;
use utils::{
    line_index::{TextRange, TextSize},
    path_identity::PathIdentityIndex,
    paths::{AbsPathBuf, Utf8Path},
};
use vfs::FileId;

use crate::base_db::source_db::{MappedSourcePreprocModel, SourcePreprocQueryError, SourceRootDb};

pub type PreprocResult<T> = Result<T, PreprocError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreprocError {
    SourceQuery(SourcePreprocQueryError),
    MissingRootSource,
    UnmappedSource { buffer_id: u32 },
}

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

impl From<SourcePreprocQueryError> for PreprocError {
    fn from(value: SourcePreprocQueryError) -> Self {
        Self::SourceQuery(value)
    }
}

pub fn visible_macros_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Vec<MacroDefinition>> {
    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;
    let position = root_position(mapped, offset)?;

    mapped
        .model
        .visible_macros_at(position)
        .into_iter()
        .filter_map(|binding| map_binding_definition(mapped, binding).transpose())
        .collect()
}

pub fn macro_definition_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroDefinition>> {
    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;

    for (define_index, define) in mapped.model.defines().iter().enumerate() {
        let (define_file_id, range) = map_source_range(mapped, define.range)?;
        if define_file_id == file_id && range_contains_offset(range, offset) {
            return Ok(Some(MacroDefinition {
                file_id: define_file_id,
                name: match define.name.clone() {
                    Some(name) => name,
                    None => return Ok(None),
                },
                define_index,
                range,
            }));
        }
    }

    Ok(None)
}

pub fn macro_usage_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroUsageResolution>> {
    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;

    for (usage_index, usage) in mapped.model.usages().iter().enumerate() {
        let (usage_file_id, range) = map_source_range(mapped, usage.range)?;
        if usage_file_id != file_id || !range_contains_offset(range, offset) {
            continue;
        }

        let Some(name) = usage.name.clone() else {
            return Ok(None);
        };
        let Some(binding) = mapped.model.definition_for_usage(usage_index) else {
            return Ok(None);
        };
        let Some(definition) = map_binding_definition(mapped, binding)? else {
            return Ok(None);
        };

        return Ok(Some(MacroUsageResolution {
            usage: MacroUsage { file_id: usage_file_id, name, usage_index, range },
            definition,
        }));
    }

    Ok(None)
}

pub fn macro_reference_resolution_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<MacroReferenceResolution>> {
    if let Some(resolution) = macro_usage_resolution_at(db, file_id, offset)? {
        return Ok(Some(MacroReferenceResolution {
            reference: MacroReference {
                file_id: resolution.usage.file_id,
                name: resolution.usage.name,
                range: resolution.usage.range,
            },
            definition: resolution.definition,
        }));
    }

    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;
    for conditional in mapped.model.conditionals() {
        for token in &conditional.expr {
            let Some(source_range) = token.range else {
                continue;
            };
            let (reference_file_id, range) = map_source_range(mapped, source_range)?;
            if reference_file_id != file_id || !range_contains_offset(range, offset) {
                continue;
            }
            let position = SourcePosition { source: source_range.source, offset: range.start() };
            let Some(definition) =
                macro_definition_for_name_at(mapped, token.value.as_str(), position)?
            else {
                return Ok(None);
            };
            return Ok(Some(MacroReferenceResolution {
                reference: MacroReference {
                    file_id: reference_file_id,
                    name: token.value.clone(),
                    range,
                },
                definition,
            }));
        }
    }

    Ok(None)
}

pub fn macro_references(
    db: &dyn SourceRootDb,
    file_id: FileId,
    definition: &MacroDefinition,
) -> PreprocResult<Vec<MacroReference>> {
    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;
    let mut refs = Vec::new();

    for (usage_index, usage) in mapped.model.usages().iter().enumerate() {
        let Some(binding) = mapped.model.definition_for_usage(usage_index) else {
            continue;
        };
        if !binding_matches_definition(mapped, &binding, definition)? {
            continue;
        }
        let (usage_file_id, range) = map_source_range(mapped, usage.range)?;
        let Some(name) = usage.name.clone() else {
            continue;
        };
        refs.push(MacroReference { file_id: usage_file_id, name, range });
    }

    for conditional in mapped.model.conditionals() {
        for token in &conditional.expr {
            let Some(source_range) = token.range else {
                continue;
            };
            let position =
                SourcePosition { source: source_range.source, offset: source_range.range.start() };
            let Some(resolved) =
                macro_definition_for_name_at(mapped, token.value.as_str(), position)?
            else {
                continue;
            };
            if resolved.file_id != definition.file_id
                || resolved.range != definition.range
                || resolved.name != definition.name
            {
                continue;
            }
            let (reference_file_id, range) = map_source_range(mapped, source_range)?;
            refs.push(MacroReference {
                file_id: reference_file_id,
                name: token.value.clone(),
                range,
            });
        }
    }

    Ok(refs)
}

pub fn include_directive_at(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> PreprocResult<Option<IncludeDirective>> {
    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;
    for (include_index, include) in mapped.model.includes().iter().enumerate() {
        let (include_file_id, range) = map_source_range(mapped, include.range)?;
        if include_file_id != file_id || !range_contains_offset(range, offset) {
            continue;
        }
        let target = match &include.target {
            ::preproc::index::MacroIncludeTarget::Literal { path, .. } => IncludeTarget::Literal {
                path: path.clone(),
                resolved_file: resolve_literal_include(db, include_file_id, path),
            },
            ::preproc::index::MacroIncludeTarget::Token { raw } => {
                IncludeTarget::Token { raw: raw.clone() }
            }
        };
        return Ok(Some(IncludeDirective {
            file_id: include_file_id,
            include_index,
            range,
            target,
        }));
    }

    Ok(None)
}

pub fn inactive_branches(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> PreprocResult<Vec<InactiveBranch>> {
    let mapped = db.source_preproc_model(file_id);
    let mapped = mapped_result(mapped.as_ref())?;
    let mut branches = Vec::new();

    for source_range in mapped.model.inactive_ranges() {
        let (branch_file_id, range) = map_source_range(mapped, *source_range)?;
        if branch_file_id == file_id {
            branches.push(InactiveBranch { file_id: branch_file_id, range });
        }
    }

    Ok(branches)
}

fn mapped_result(
    result: &Result<MappedSourcePreprocModel, SourcePreprocQueryError>,
) -> PreprocResult<&MappedSourcePreprocModel> {
    result.as_ref().map_err(|err| err.clone().into())
}

fn root_position(
    mapped: &MappedSourcePreprocModel,
    offset: TextSize,
) -> PreprocResult<SourcePosition> {
    let source = mapped.model.root_source().ok_or(PreprocError::MissingRootSource)?;
    Ok(SourcePosition { source, offset })
}

fn map_source_range(
    mapped: &MappedSourcePreprocModel,
    source_range: SourceRange,
) -> PreprocResult<(FileId, TextRange)> {
    let file_id = mapped
        .source_file_ids
        .get(&source_range.source)
        .copied()
        .ok_or_else(|| PreprocError::UnmappedSource { buffer_id: source_range.source.raw() })?;
    Ok((file_id, source_range.range))
}

fn map_binding_definition(
    mapped: &MappedSourcePreprocModel,
    binding: SourceMacroBinding<'_>,
) -> PreprocResult<Option<MacroDefinition>> {
    let (file_id, range) = map_source_range(mapped, binding.define.range)?;
    let Some(name) = binding.define.name.clone() else {
        return Ok(None);
    };
    Ok(Some(MacroDefinition { file_id, name, define_index: binding.define_index, range }))
}

fn binding_matches_definition(
    mapped: &MappedSourcePreprocModel,
    binding: &SourceMacroBinding<'_>,
    definition: &MacroDefinition,
) -> PreprocResult<bool> {
    let Some(mapped_definition) = map_binding_definition(mapped, (*binding).clone())? else {
        return Ok(false);
    };
    Ok(mapped_definition.file_id == definition.file_id
        && mapped_definition.range == definition.range
        && mapped_definition.name == definition.name)
}

fn resolve_literal_include(db: &dyn SourceRootDb, file_id: FileId, path: &str) -> Option<FileId> {
    let includer_path = db.file_path(file_id)?;
    let include_dirs = db.file_preprocess_config(file_id).include_dirs.clone();
    let path_file_ids = path_file_ids(db);
    resolve_include_target(path, &includer_path, &include_dirs, &path_file_ids)
}

fn macro_definition_for_name_at(
    mapped: &MappedSourcePreprocModel,
    name: &str,
    position: SourcePosition,
) -> PreprocResult<Option<MacroDefinition>> {
    for binding in mapped.model.visible_macros_at(position) {
        if binding.name.as_str() != name {
            continue;
        }
        return map_binding_definition(mapped, binding);
    }
    Ok(None)
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
