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

#[cfg(test)]
mod tests {
    use std::fmt;

    use rustc_hash::FxHashSet;
    use triomphe::Arc;
    use utils::{
        line_index::TextSize,
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

    use super::*;
    use crate::base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        salsa::{self, Durability},
        source_db::{
            FileLoader, SourceDb, SourceDbStorage, SourceFileKind, SourceRootDb,
            SourceRootDbStorage,
        },
        source_root::{SourceRoot, SourceRootId},
    };

    const TOP: FileId = FileId(0);
    const HEADER: FileId = FileId(1);
    const ROOT: SourceRootId = SourceRootId(0);
    const PROFILE: CompilationProfileId = CompilationProfileId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage)]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    impl salsa::Database for TestDb {}

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_files(root_text: &str, header_text: &str) -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let header_path = abs_path("include/defs.vh");
        let include_dir = abs_path("include");

        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        file_set.insert(HEADER, VfsPath::from(header_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);

        let preprocess =
            PreprocessConfig { predefines: Vec::new(), include_dirs: vec![include_dir.clone()] };
        let project_config = ProjectConfig::new(
            vec![Some(PROFILE)],
            vec![CompilationProfile {
                source_roots: vec![ROOT],
                top_modules: Vec::new(),
                preprocess: preprocess.clone(),
            }],
        );

        let mut files = FxHashSet::default();
        files.insert(TOP);
        files.insert(HEADER);

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);

        for (file_id, path, text) in
            [(TOP, top_path, root_text), (HEADER, header_path, header_text)]
        {
            let vfs_path = VfsPath::from(path.clone());
            db.set_source_root_id_with_durability(file_id, ROOT, Durability::LOW);
            db.set_file_path_with_durability(file_id, Some(path), Durability::LOW);
            db.set_file_kind_with_durability(
                file_id,
                SourceFileKind::from_path(&vfs_path),
                Durability::LOW,
            );
            db.set_file_text_with_durability(file_id, Arc::from(text), Durability::LOW);
            db.set_file_preprocess_config_with_durability(
                file_id,
                Arc::new(preprocess.clone()),
                Durability::LOW,
            );
        }

        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
    }

    fn text_at_range(text: &str, range: TextRange) -> &str {
        &text[usize::from(range.start())..usize::from(range.end())]
    }

    #[test]
    fn preproc_include_usage_resolves_to_header_define() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
        let header_text = "`define HEADER_WIDTH 8\n";
        let db = db_with_files(root_text, header_text);

        let resolution = macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH"))
            .unwrap()
            .unwrap();
        assert_eq!(resolution.usage.file_id, TOP);
        assert_eq!(resolution.definition.file_id, HEADER);
        assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
        assert!(text_at_range(header_text, resolution.definition.range).contains("HEADER_WIDTH"));

        let include =
            include_directive_at(&db, TOP, offset(root_text, "defs.vh")).unwrap().unwrap();
        let IncludeTarget::Literal { resolved_file, .. } = include.target else {
            panic!("literal include expected");
        };
        assert_eq!(resolved_file, Some(HEADER));
    }

    #[test]
    fn preproc_unsaved_include_buffer_updates_query_result() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `HEADER_WIDTH;
endmodule
"#;
        let mut db = db_with_files(root_text, "`define OTHER_WIDTH 8\n");

        assert!(
            macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH"))
                .unwrap()
                .is_none()
        );

        db.set_file_text_with_durability(
            HEADER,
            Arc::from("`define HEADER_WIDTH 16\n"),
            Durability::LOW,
        );

        let resolution = macro_usage_resolution_at(&db, TOP, offset(root_text, "HEADER_WIDTH"))
            .unwrap()
            .unwrap();
        assert_eq!(resolution.definition.file_id, HEADER);
        assert_eq!(resolution.definition.name.as_str(), "HEADER_WIDTH");
    }

    #[test]
    fn preproc_inactive_branch_uses_header_define() {
        let root_text = r#"`include "defs.vh"
`ifndef HEADER_FLAG
wire disabled_by_header;
`endif
wire active;
"#;
        let header_text = "`define HEADER_FLAG\n";
        let db = db_with_files(root_text, header_text);

        let branches = inactive_branches(&db, TOP).unwrap();
        assert_eq!(branches.len(), 1);
        assert_eq!(branches[0].file_id, TOP);
        assert!(text_at_range(root_text, branches[0].range).contains("disabled_by_header"));
    }
}
