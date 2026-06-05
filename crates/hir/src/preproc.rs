use preproc::source::{
    MacroIncludeTarget, SourceIncludeChainEntry, SourceMacroBinding, SourcePosition,
    SourcePreprocError, SourcePreprocProvenance, SourceRange,
};
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
    pub event_id: u32,
    pub event_range: TextRange,
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
    pub definition_provenance: MacroDefinitionProvenance,
    pub include_chain: Vec<IncludeChainEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefinitionProvenance {
    pub event_id: u32,
    pub file_id: FileId,
    pub range: TextRange,
    pub name_range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeChainEntry {
    pub include_event_id: u32,
    pub include_file_id: FileId,
    pub include_range: TextRange,
    pub included_file_id: FileId,
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

impl From<SourcePreprocError> for PreprocError {
    fn from(value: SourcePreprocError) -> Self {
        Self::SourceQuery(SourcePreprocQueryError::Model(value))
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
        let (define_file_id, event_range) = map_source_range(mapped, define.range)?;
        let (_, range) = map_source_range(mapped, define.name_range.unwrap_or(define.range))?;
        if define_file_id == file_id && range_contains_offset(range, offset) {
            return Ok(Some(MacroDefinition {
                file_id: define_file_id,
                name: match define.name.clone() {
                    Some(name) => name,
                    None => return Ok(None),
                },
                define_index,
                event_id: define.event_id.raw(),
                event_range,
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
        let Some(source_resolution) = mapped.model.definition_for_usage(usage_index)? else {
            return Ok(None);
        };
        let Some(definition) = map_binding_definition(mapped, source_resolution.definition)? else {
            return Ok(None);
        };
        let definition_provenance =
            map_definition_provenance(mapped, &source_resolution.definition_provenance)?;
        let include_chain = map_include_chain(mapped, &source_resolution.definition_include_chain)?;

        return Ok(Some(MacroUsageResolution {
            usage: MacroUsage { file_id: usage_file_id, name, usage_index, range },
            definition,
            definition_provenance,
            include_chain,
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
    for (conditional_index, conditional) in mapped.model.conditionals().iter().enumerate() {
        for (token_index, token) in conditional.expr.iter().enumerate() {
            let Some(source_range) = token.range else {
                continue;
            };
            let (reference_file_id, range) = map_source_range(mapped, source_range)?;
            if reference_file_id != file_id || !range_contains_offset(range, offset) {
                continue;
            }
            let Some(source_definition) =
                mapped.model.definition_for_conditional_token(conditional_index, token_index)
            else {
                return Ok(None);
            };
            let Some(definition) = map_binding_definition(mapped, source_definition)? else {
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
    let mut refs = Vec::new();

    for model_file_id in preproc_model_file_ids(db, file_id) {
        let mapped = db.source_preproc_model(model_file_id);
        let Ok(mapped) = mapped.as_ref() else {
            continue;
        };
        collect_macro_references_in_model(mapped, definition, &mut refs)?;
    }

    Ok(refs)
}

fn collect_macro_references_in_model(
    mapped: &MappedSourcePreprocModel,
    definition: &MacroDefinition,
    refs: &mut Vec<MacroReference>,
) -> PreprocResult<()> {
    for (usage_index, usage) in mapped.model.usages().iter().enumerate() {
        let Some(source_resolution) = mapped.model.definition_for_usage(usage_index)? else {
            continue;
        };
        if !binding_matches_definition(mapped, &source_resolution.definition, definition)? {
            continue;
        }
        let (usage_file_id, range) = map_source_range(mapped, usage.range)?;
        let Some(name) = usage.name.clone() else {
            continue;
        };
        push_unique_macro_reference(refs, MacroReference { file_id: usage_file_id, name, range });
    }

    for (conditional_index, conditional) in mapped.model.conditionals().iter().enumerate() {
        for (token_index, token) in conditional.expr.iter().enumerate() {
            let Some(source_range) = token.range else {
                continue;
            };
            let Some(resolved) =
                mapped.model.definition_for_conditional_token(conditional_index, token_index)
            else {
                continue;
            };
            if !binding_matches_definition(mapped, &resolved, definition)? {
                continue;
            }
            let (reference_file_id, range) = map_source_range(mapped, source_range)?;
            push_unique_macro_reference(
                refs,
                MacroReference { file_id: reference_file_id, name: token.value.clone(), range },
            );
        }
    }

    Ok(())
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
            MacroIncludeTarget::Literal { path, .. } => IncludeTarget::Literal {
                path: path.clone(),
                resolved_file: resolve_literal_include(db, include_file_id, path),
            },
            MacroIncludeTarget::Token { raw } => IncludeTarget::Token { raw: raw.clone() },
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
    let file_id = map_source_id(mapped, source_range.source)?;
    Ok((file_id, source_range.range))
}

fn map_source_id(
    mapped: &MappedSourcePreprocModel,
    source: preproc::source::PreprocSourceId,
) -> PreprocResult<FileId> {
    mapped
        .source_file_ids
        .get(&source)
        .copied()
        .ok_or_else(|| PreprocError::UnmappedSource { buffer_id: source.raw() })
}

fn map_definition_provenance(
    mapped: &MappedSourcePreprocModel,
    provenance: &SourcePreprocProvenance,
) -> PreprocResult<MacroDefinitionProvenance> {
    let (file_id, range) = map_source_range(mapped, provenance.range)?;
    let name_range = provenance
        .name_range
        .map(|source_range| map_source_range(mapped, source_range).map(|(_, range)| range))
        .transpose()?;
    Ok(MacroDefinitionProvenance {
        event_id: provenance.event_id.raw(),
        file_id,
        range,
        name_range,
    })
}

fn map_include_chain(
    mapped: &MappedSourcePreprocModel,
    chain: &[SourceIncludeChainEntry],
) -> PreprocResult<Vec<IncludeChainEntry>> {
    chain
        .iter()
        .map(|entry| {
            let (include_file_id, include_range) = map_source_range(mapped, entry.include_range)?;
            let included_file_id = map_source_id(mapped, entry.included_source)?;
            Ok(IncludeChainEntry {
                include_event_id: entry.include_event_id.raw(),
                include_file_id,
                include_range,
                included_file_id,
            })
        })
        .collect()
}

fn map_binding_definition(
    mapped: &MappedSourcePreprocModel,
    binding: SourceMacroBinding<'_>,
) -> PreprocResult<Option<MacroDefinition>> {
    let (file_id, event_range) = map_source_range(mapped, binding.define.range)?;
    let (_, range) =
        map_source_range(mapped, binding.define.name_range.unwrap_or(binding.define.range))?;
    let Some(name) = binding.define.name.clone() else {
        return Ok(None);
    };
    Ok(Some(MacroDefinition {
        file_id,
        name,
        define_index: binding.define_index,
        event_id: binding.event_id.raw(),
        event_range,
        range,
    }))
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

fn push_unique_macro_reference(refs: &mut Vec<MacroReference>, reference: MacroReference) {
    if refs.iter().any(|existing| {
        existing.file_id == reference.file_id
            && existing.range == reference.range
            && existing.name == reference.name
    }) {
        return;
    }
    refs.push(reference);
}

fn preproc_model_file_ids(db: &dyn SourceRootDb, file_id: FileId) -> Vec<FileId> {
    let mut file_ids = vec![file_id];
    file_ids.extend(
        db.files()
            .iter()
            .copied()
            .filter(|candidate| *candidate != file_id && !db.file_is_project_ignored(*candidate)),
    );
    file_ids.sort();
    file_ids.dedup();
    file_ids
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
    const LEAF: FileId = FileId(2);
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
        db_with_entries(&[(TOP, "rtl/top.v", root_text), (HEADER, "include/defs.vh", header_text)])
    }

    fn db_with_nested_files(root_text: &str, header_text: &str, leaf_text: &str) -> TestDb {
        db_with_entries(&[
            (TOP, "rtl/top.v", root_text),
            (HEADER, "include/defs.vh", header_text),
            (LEAF, "include/leaf.vh", leaf_text),
        ])
    }

    fn db_with_entries(entries: &[(FileId, &str, &str)]) -> TestDb {
        let include_dir = abs_path("include");

        let mut file_set = FileSet::default();
        for (file_id, path, _) in entries {
            file_set.insert(*file_id, VfsPath::from(abs_path(path)));
        }
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
        for (file_id, _, _) in entries {
            files.insert(*file_id);
        }

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(project_config), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);

        for (file_id, path, text) in entries {
            let path = abs_path(path);
            let vfs_path = VfsPath::from(path.clone());
            db.set_source_root_id_with_durability(*file_id, ROOT, Durability::LOW);
            db.set_file_path_with_durability(*file_id, Some(path), Durability::LOW);
            db.set_file_kind_with_durability(
                *file_id,
                SourceFileKind::from_path(&vfs_path),
                Durability::LOW,
            );
            db.set_file_text_with_durability(*file_id, Arc::from(*text), Durability::LOW);
            db.set_file_preprocess_config_with_durability(
                *file_id,
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

    fn offset_after(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).unwrap() + needle.len()).unwrap())
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
    fn preproc_nested_include_chain_maps_to_file_ids() {
        let root_text = r#"`include "defs.vh"
module top;
localparam int W = `LEAF_WIDTH;
endmodule
"#;
        let header_text = "`include \"leaf.vh\"\n";
        let leaf_text = "`define LEAF_WIDTH 4\n";
        let db = db_with_nested_files(root_text, header_text, leaf_text);

        let resolution =
            macro_usage_resolution_at(&db, TOP, offset(root_text, "LEAF_WIDTH")).unwrap().unwrap();

        assert_eq!(resolution.definition.file_id, LEAF);
        assert_eq!(resolution.definition_provenance.file_id, LEAF);
        assert_eq!(resolution.include_chain.len(), 2);
        assert_eq!(resolution.include_chain[0].include_file_id, TOP);
        assert_eq!(resolution.include_chain[0].included_file_id, HEADER);
        assert!(
            text_at_range(root_text, resolution.include_chain[0].include_range).contains("defs.vh")
        );
        assert_eq!(resolution.include_chain[1].include_file_id, HEADER);
        assert_eq!(resolution.include_chain[1].included_file_id, LEAF);
        assert!(
            text_at_range(header_text, resolution.include_chain[1].include_range)
                .contains("leaf.vh")
        );
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

    #[test]
    fn preproc_included_define_references_include_root_conditionals() {
        let root_text = r#"`include "defs.vh"
`ifdef HEADER_FLAG
localparam int ENABLED = `HEADER_FLAG;
`endif
"#;
        let header_text = "`define HEADER_FLAG 1\n";
        let db = db_with_files(root_text, header_text);
        let definition = macro_definition_at(&db, HEADER, offset_after(header_text, "`define "))
            .unwrap()
            .unwrap();

        let refs = macro_references(&db, HEADER, &definition).unwrap();

        assert!(refs.iter().any(|reference| {
            reference.file_id == TOP && text_at_range(root_text, reference.range) == "HEADER_FLAG"
        }));
        assert!(refs.iter().any(|reference| {
            reference.file_id == TOP && text_at_range(root_text, reference.range) == "`HEADER_FLAG"
        }));
    }

    #[test]
    fn preproc_ifndef_guard_reference_resolves_to_following_define() {
        let root_text = "`include \"defs.vh\"\n";
        let header_text = r#"`ifndef HEADER_FLAG
`define HEADER_FLAG
`endif
"#;
        let db = db_with_files(root_text, header_text);
        let resolution =
            macro_reference_resolution_at(&db, HEADER, offset(header_text, "HEADER_FLAG"))
                .unwrap()
                .unwrap();

        assert_eq!(resolution.reference.file_id, HEADER);
        assert_eq!(resolution.definition.file_id, HEADER);
        assert_eq!(text_at_range(header_text, resolution.definition.range), "HEADER_FLAG");

        let refs = macro_references(&db, HEADER, &resolution.definition).unwrap();
        assert!(refs.iter().any(|reference| {
            reference.file_id == HEADER
                && reference.range.start() == offset(header_text, "HEADER_FLAG")
                && text_at_range(header_text, reference.range) == "HEADER_FLAG"
        }));
    }
}
