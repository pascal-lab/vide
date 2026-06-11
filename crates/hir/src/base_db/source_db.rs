use rustc_hash::{FxHashMap, FxHashSet};
use syntax::{
    Compilation, ParserExpectedSyntax, PreprocessorTrace, SyntaxDiagnostic, SyntaxTree,
    SyntaxTreeBuffer, SyntaxTreeBufferIds,
};
use triomphe::Arc;
use utils::{line_index::TextSize, path_identity::PathIdentityIndex};
use vfs::{FileId, VfsPath, anchored_path::AnchoredPath};

use crate::base_db::{
    compilation_plan::{self, CompilationPlan},
    diagnostics_config::{DiagnosticSource, DiagnosticsConfig},
    project::{CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_root::{SourceRoot, SourceRootId},
};

mod preproc;

pub use self::preproc::{
    MappedSourcePreprocModel, PreprocExpansionDisplay, PreprocExpansionMapping,
    PreprocExpansionSourceBuffer, PreprocManifestSource, PreprocSourceMap, PreprocSourceMapError,
    PreprocSourceMapping, PreprocSpeculativeUniverseId, PreprocVirtualOrigin,
    SourceGraphPreprocModel, SourcePreprocContextIndex, SourcePreprocContextStatus,
    SourcePreprocQueryError, SourcePreprocRelevantContexts, preproc_virtual_builtin_path,
    preproc_virtual_expansion_path, preproc_virtual_predefines_path,
    preproc_virtual_speculative_path,
};
#[cfg(test)]
use self::preproc::{
    materialized_predefine_text, source_preproc_file_ids, workspace_preproc_model_file_ids,
};
use self::preproc::{
    source_graph_preproc_model, source_preproc_context_index_for_profile,
    source_preproc_contexts_for_file, source_preproc_model,
};

pub trait FileLoader {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId>;
}

// Source code, syntax tree and project model.
// Everything else is derived from these queries.
#[salsa::query_group(SourceDbStorage)]
pub trait SourceDb: FileLoader + std::fmt::Debug {
    #[salsa::input]
    fn file_text(&self, file_id: FileId) -> Arc<str>;

    #[salsa::input]
    fn file_kind(&self, file_id: FileId) -> SourceFileKind;

    #[salsa::input]
    fn file_path(&self, file_id: FileId) -> Option<utils::paths::AbsPathBuf>;

    #[salsa::input]
    fn file_preprocess_config(&self, file_id: FileId) -> Arc<PreprocessConfig>;

    fn parse_src(&self, file_id: FileId) -> SyntaxTree;

    #[salsa::input]
    fn files(&self) -> Box<FxHashSet<FileId>>;

    #[salsa::input]
    fn diagnostics_config(&self) -> Arc<DiagnosticsConfig>;

    #[salsa::input]
    fn project_config(&self) -> Arc<ProjectConfig>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SourceFileKind {
    #[default]
    SystemVerilog,
    IncludeHeader,
    LibraryMap,
    ProjectManifest,
}

impl SourceFileKind {
    pub fn from_path(path: &VfsPath) -> Self {
        match path.name_and_extension() {
            Some((name, Some(ext))) if name == "vide" && ext.eq_ignore_ascii_case("toml") => {
                Self::ProjectManifest
            }
            Some((_, Some(ext))) if ext.eq_ignore_ascii_case("map") => Self::LibraryMap,
            Some((_, Some(ext)))
                if ["vh", "svh", "svi"].iter().any(|header| ext.eq_ignore_ascii_case(header)) =>
            {
                Self::IncludeHeader
            }
            _ => Self::SystemVerilog,
        }
    }

    pub(crate) fn is_semantic_compilation_unit(self) -> bool {
        matches!(self, Self::SystemVerilog | Self::LibraryMap)
    }

    fn is_slang_parse_unit(self) -> bool {
        matches!(self, Self::SystemVerilog | Self::LibraryMap)
    }
}

fn parse_src(db: &dyn SourceDb, file_id: FileId) -> SyntaxTree {
    let _span = tracing::info_span!("slang.parse_src", ?file_id).entered();
    let text = db.file_text(file_id);

    match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            // HIR source maps are local to the queried file; project-aware
            // include expansion belongs to parse_src_for_compilation.
            let preprocess = db.file_preprocess_config(file_id);
            let include_paths = preprocess.include_dir_strings();
            let options = syntax::SyntaxTreeOptions {
                predefines: preprocess.predefine_strings(),
                include_paths,
                ..syntax::SyntaxTreeOptions::without_include_expansion()
            };
            let _span = tracing::info_span!(
                "slang.syntax_tree.from_text",
                ?file_id,
                bytes = text.len(),
                include_buffer_count = 0usize
            )
            .entered();
            SyntaxTree::from_text_with_options(&text, "", "", &options)
        }
        SourceFileKind::LibraryMap => SyntaxTree::from_library_map_text(&text, "", ""),
        SourceFileKind::ProjectManifest => SyntaxTree::from_text("", "", ""),
    }
}

struct SourceFileIdentity {
    name: String,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationDiagnostic {
    /// File attribution after mapping slang source buffers back to VFS files.
    pub file_id: FileId,
    /// The compilation phase that produced the diagnostic.
    pub source: DiagnosticSource,
    pub diagnostic: SyntaxDiagnostic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCompilationUnit {
    pub syntax_tree: SyntaxTree,
    pub preprocessor_trace: Option<PreprocessorTrace>,
}

fn source_file_identity(db: &dyn SourceDb, file_id: FileId) -> SourceFileIdentity {
    let path = db.file_path(file_id).map(|path| path.to_string()).unwrap_or_default();
    let name = if path.is_empty() { "source".to_owned() } else { path.clone() };
    SourceFileIdentity { name, path }
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

fn insert_buffer_file_ids(
    buffer_file_ids: &mut FxHashMap<u32, FileId>,
    path_file_ids: &PathIdentityIndex<FileId>,
    buffers: SyntaxTreeBufferIds,
    root_file_id: FileId,
) {
    buffer_file_ids.insert(buffers.root_buffer_id, root_file_id);
    for buffer in buffers.source_buffers {
        if let Some(file_id) = path_file_ids.get(&buffer.path) {
            buffer_file_ids.insert(buffer.buffer_id, file_id);
        }
    }
}

fn syntax_tree_options_for_file(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    let _span = tracing::info_span!("slang.syntax_tree_options.file", ?file_id).entered();
    let preprocess = db.file_preprocess_config(file_id);
    let profile_id = db.file_compilation_profile(file_id);
    let include_buffers = db.include_buffers_for_profile(profile_id).as_ref().clone();
    syntax::SyntaxTreeOptions {
        predefines: preprocess.predefine_strings(),
        include_paths: preprocess.include_dir_strings(),
        include_buffers,
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn syntax_tree_options_for_profile(
    project_config: &ProjectConfig,
    profile_id: Option<CompilationProfileId>,
    include_buffers: Vec<SyntaxTreeBuffer>,
) -> syntax::SyntaxTreeOptions {
    let preprocess = project_config.preprocess_for_profile(profile_id);
    let include_paths = preprocess.include_dir_strings();
    syntax::SyntaxTreeOptions {
        predefines: preprocess.predefine_strings(),
        include_paths,
        include_buffers,
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn parsed_compilation_unit(db: &dyn SourceRootDb, file_id: FileId) -> ParsedCompilationUnit {
    let _span = tracing::info_span!("slang.parse_for_compilation", ?file_id).entered();
    let text = {
        let _span =
            tracing::info_span!("slang.parse_for_compilation.file_text", ?file_id).entered();
        db.file_text(file_id)
    };
    let identity = source_file_identity(db, file_id);

    match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let options = syntax_tree_options_for_file(db, file_id);
            let include_buffer_count = options.include_buffers.len();
            let _span = tracing::info_span!(
                "slang.parse_for_compilation.from_text",
                ?file_id,
                bytes = text.len(),
                include_buffer_count
            )
            .entered();
            let parsed = SyntaxTree::from_text_with_options_and_trace(
                &text,
                &identity.name,
                &identity.path,
                &options,
            );
            ParsedCompilationUnit {
                syntax_tree: parsed.tree,
                preprocessor_trace: parsed.preprocessor_trace,
            }
        }
        SourceFileKind::LibraryMap => ParsedCompilationUnit {
            syntax_tree: SyntaxTree::from_library_map_text(&text, &identity.name, &identity.path),
            preprocessor_trace: None,
        },
        SourceFileKind::ProjectManifest => ParsedCompilationUnit {
            syntax_tree: SyntaxTree::from_text("", "", ""),
            preprocessor_trace: None,
        },
    }
}

fn parse_src_for_compilation(db: &dyn SourceRootDb, file_id: FileId) -> SyntaxTree {
    db.parsed_compilation_unit(file_id).syntax_tree.clone()
}

fn parser_expected_syntax(
    db: &dyn SourceRootDb,
    file_id: FileId,
    offset: TextSize,
) -> Arc<[ParserExpectedSyntax]> {
    if matches!(db.file_kind(file_id), SourceFileKind::ProjectManifest) {
        return Arc::from(Vec::<ParserExpectedSyntax>::new());
    }

    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let offset = usize::from(offset);
    let expected = match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let options = syntax_tree_options_for_file(db, file_id);
            SyntaxTree::expected_syntax_at_offset_with_options(
                &text,
                &identity.name,
                &identity.path,
                offset,
                &options,
            )
        }
        SourceFileKind::LibraryMap => SyntaxTree::library_map_expected_syntax_at_offset(
            &text,
            &identity.name,
            &identity.path,
            offset,
        ),
        SourceFileKind::ProjectManifest => Vec::new(),
    };
    Arc::from(expected)
}

fn parse_diagnostics(db: &dyn SourceRootDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    let config = db.diagnostics_config();
    if !config.enabled || !config.parse.enabled || !db.file_kind(file_id).is_slang_parse_unit() {
        return Arc::from(Vec::<SyntaxDiagnostic>::new());
    }

    let _span = tracing::info_span!("slang.parse_diagnostics", ?file_id).entered();
    let tree = {
        let _span = tracing::info_span!("slang.parse_diagnostics.parse_tree", ?file_id).entered();
        db.parse_src_for_compilation(file_id)
    };
    let root_buffer_id = tree.buffer_id();
    let raw_diagnostics = {
        let _span = tracing::info_span!("slang.parse.raw_diagnostics", ?file_id).entered();
        tree.diagnostics_with_options(&config.slang.warnings)
    };
    let raw_diagnostic_count = raw_diagnostics.len();
    let mut non_root_buffer_count = 0usize;
    let mut ignored_diagnostic_count = 0usize;
    let mut diags = Vec::new();

    for diag in raw_diagnostics {
        if !diag.buffer_id.is_none_or(|buffer_id| buffer_id == root_buffer_id) {
            non_root_buffer_count += 1;
            continue;
        }

        match config.apply_rules(DiagnosticSource::Parse, diag) {
            Some(diag) => diags.push(diag),
            None => ignored_diagnostic_count += 1,
        }
    }

    tracing::info!(
        raw_diagnostic_count,
        non_root_buffer_count,
        ignored_diagnostic_count,
        diagnostic_count = diags.len(),
        "parse diagnostics complete"
    );
    Arc::from(diags)
}

// Don't expose source roots to HIR, so extract them in a separate DB.
#[salsa::query_group(SourceRootDbStorage)]
pub trait SourceRootDb: SourceDb {
    #[salsa::input]
    fn source_root_id(&self, file_id: FileId) -> SourceRootId;

    #[salsa::input]
    fn source_root(&self, id: SourceRootId) -> Arc<SourceRoot>;

    fn file_compilation_profile(&self, file_id: FileId) -> Option<CompilationProfileId>;
    fn file_is_project_ignored(&self, file_id: FileId) -> bool;
    fn compilation_plan_for_root(&self, source_root_id: SourceRootId) -> Arc<CompilationPlan>;
    fn compilation_plan_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationPlan>;
    /// Diagnostics produced by one slang compilation profile. This is the
    /// semantic diagnostics path, but it also returns parse diagnostics from
    /// the same syntax trees so one request does not parse the same roots
    /// twice.
    fn compilation_profile_diagnostics(
        &self,
        profile_id: CompilationProfileId,
    ) -> Arc<[CompilationDiagnostic]>;
    fn include_buffers_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<Vec<SyntaxTreeBuffer>>;
    fn source_preproc_model(
        &self,
        file_id: FileId,
    ) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>>;
    fn source_graph_preproc_model(
        &self,
        file_id: FileId,
    ) -> Arc<Result<SourceGraphPreprocModel, SourcePreprocQueryError>>;
    fn source_preproc_context_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<SourcePreprocContextIndex>;
    fn source_preproc_contexts_for_file(
        &self,
        file_id: FileId,
    ) -> Arc<SourcePreprocRelevantContexts>;
    fn parsed_compilation_unit(&self, file_id: FileId) -> ParsedCompilationUnit;
    fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree;
    fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Arc<[ParserExpectedSyntax]>;
    fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    /// Diagnostics for the compilation profile that owns `file_id`.
    fn file_compilation_diagnostics(&self, file_id: FileId) -> Arc<[CompilationDiagnostic]>;
    fn semantic_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]>;
    fn source_root_semantic_diagnostics(
        &self,
        file_id: FileId,
    ) -> Arc<[(FileId, SyntaxDiagnostic)]>;
}

fn file_compilation_profile(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Option<CompilationProfileId> {
    let source_root_id = db.source_root_id(file_id);
    let project_config = db.project_config();
    let profile_id = project_config.profile_for_root(source_root_id);
    let source_root = db.source_root(source_root_id);
    if profile_id.is_none() && source_root.role().reports_missing_profile() {
        tracing::debug!(
            ?file_id,
            ?source_root_id,
            root_profile_count = project_config.root_profile_count(),
            "file has no compilation profile",
        );
    }
    profile_id
}

fn file_is_project_ignored(db: &dyn SourceRootDb, file_id: FileId) -> bool {
    let source_root_id = db.source_root_id(file_id);
    db.source_root(source_root_id).is_ignored()
}

fn compilation_plan_for_root(
    db: &dyn SourceRootDb,
    source_root_id: SourceRootId,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_source_root(db, source_root_id))
}

fn compilation_plan_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_profile(db, profile_id))
}

fn include_buffers_for_profile(
    db: &dyn SourceRootDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<Vec<SyntaxTreeBuffer>> {
    let plan = db.compilation_plan_for_profile(profile_id);
    Arc::new(compilation_plan::include_buffers_for_plan(db, &plan))
}

fn semantic_diagnostics(db: &dyn SourceRootDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
    Arc::from(
        db.source_root_semantic_diagnostics(file_id)
            .iter()
            .filter_map(|(diag_file_id, diag)| (*diag_file_id == file_id).then_some(diag.clone()))
            .collect::<Vec<_>>(),
    )
}

fn file_compilation_diagnostics(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<[CompilationDiagnostic]> {
    let source_root_id = db.source_root_id(file_id);
    let config = db.diagnostics_config();
    if !config.enabled || !config.semantic.enabled || db.file_is_project_ignored(file_id) {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    }

    let project_config = db.project_config();
    let Some(profile_id) = project_config.profile_for_root(source_root_id) else {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    };
    db.compilation_profile_diagnostics(profile_id)
}

fn compilation_profile_diagnostics(
    db: &dyn SourceRootDb,
    profile_id: CompilationProfileId,
) -> Arc<[CompilationDiagnostic]> {
    let config = db.diagnostics_config();
    if !config.enabled || !config.semantic.enabled {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    }

    let project_config = db.project_config();
    let plan = db.compilation_plan_for_profile(Some(profile_id));
    let compilation_include_buffers = {
        let _span = tracing::info_span!("slang.semantic.compilation_buffers").entered();
        compilation_plan::compilation_source_buffers_for_plan(db, &plan)
    };
    let root_count = plan.roots.len();
    let top_module_count = plan.top_modules.len();
    let include_buffer_count = compilation_include_buffers.len();
    let _span = tracing::info_span!(
        "slang.compilation_profile_diagnostics",
        ?profile_id,
        root_count,
        top_module_count,
        include_buffer_count
    )
    .entered();
    let compilation_options = syntax_tree_options_for_profile(
        &project_config,
        Some(profile_id),
        compilation_include_buffers,
    );
    let mut compilation = Compilation::new_with_top_modules(&plan.top_modules);
    let mut buffer_file_ids = FxHashMap::default();
    let path_file_ids = path_file_ids(db);
    let mut compilation_root_count = 0usize;
    let mut compilation_buffer_count = 0usize;
    {
        let _span = tracing::info_span!("slang.semantic.add_roots", root_count).entered();
        for file_id in plan.roots.iter().copied() {
            let text = {
                let _span =
                    tracing::info_span!("slang.semantic.add_root.file_text", ?file_id).entered();
                db.file_text(file_id)
            };
            let identity = source_file_identity(db, file_id);
            let buffer_ids = match db.file_kind(file_id) {
                SourceFileKind::SystemVerilog => {
                    let include_buffer_count = compilation_options.include_buffers.len();
                    let _span = tracing::info_span!(
                        "slang.semantic.add_root.from_text",
                        ?file_id,
                        bytes = text.len(),
                        include_buffer_count
                    )
                    .entered();
                    compilation.add_syntax_tree_from_text(
                        &text,
                        &identity.name,
                        &identity.path,
                        &compilation_options,
                    )
                }
                SourceFileKind::LibraryMap => compilation.add_library_map_syntax_tree_from_text(
                    &text,
                    &identity.name,
                    &identity.path,
                ),
                SourceFileKind::IncludeHeader | SourceFileKind::ProjectManifest => continue,
            };
            compilation_root_count += 1;
            compilation_buffer_count += 1 + buffer_ids.source_buffers.len();
            insert_buffer_file_ids(&mut buffer_file_ids, &path_file_ids, buffer_ids, file_id);
        }
    }
    tracing::info!(
        compilation_root_count,
        compilation_buffer_count,
        mapped_buffer_count = buffer_file_ids.len(),
        "semantic compilation roots added"
    );

    let mut diagnostics = Vec::new();
    if config.parse.enabled {
        let raw_diagnostics = {
            let _span = tracing::info_span!("slang.semantic.parse_diagnostics").entered();
            compilation.parse_diagnostics_with_options(&config.slang.warnings)
        };
        let raw_diagnostic_count = raw_diagnostics.len();
        let mut unmapped_buffer_count = 0usize;
        let mut ignored_diagnostic_count = 0usize;
        {
            let _span =
                tracing::info_span!("slang.semantic.map_parse_diagnostics", raw_diagnostic_count)
                    .entered();
            diagnostics.extend(raw_diagnostics.into_iter().filter_map(|diag| {
                let diag_file_id = match diag
                    .buffer_id
                    .and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied())
                {
                    Some(file_id) => file_id,
                    None => {
                        unmapped_buffer_count += 1;
                        return None;
                    }
                };
                let diag = match config.apply_rules(DiagnosticSource::Parse, diag) {
                    Some(diag) => diag,
                    None => {
                        ignored_diagnostic_count += 1;
                        return None;
                    }
                };
                Some(CompilationDiagnostic {
                    file_id: diag_file_id,
                    source: DiagnosticSource::Parse,
                    diagnostic: diag,
                })
            }));
        }
        tracing::info!(
            raw_diagnostic_count,
            unmapped_buffer_count,
            ignored_diagnostic_count,
            diagnostic_count = diagnostics.len(),
            "compilation parse diagnostics complete"
        );
    }

    let raw_semantic_diagnostics = {
        let _span = tracing::info_span!("slang.semantic.raw_diagnostics").entered();
        compilation.semantic_diagnostics_with_options(&config.slang.warnings)
    };
    let raw_semantic_diagnostic_count = raw_semantic_diagnostics.len();
    let mut unmapped_semantic_buffer_count = 0usize;
    let mut ignored_semantic_diagnostic_count = 0usize;
    {
        let _span =
            tracing::info_span!("slang.semantic.map_diagnostics", raw_semantic_diagnostic_count)
                .entered();
        diagnostics.extend(raw_semantic_diagnostics.into_iter().filter_map(|diag| {
            let diag_file_id =
                diag.buffer_id.and_then(|buffer_id| buffer_file_ids.get(&buffer_id).copied());
            let Some(diag_file_id) = diag_file_id else {
                unmapped_semantic_buffer_count += 1;
                return None;
            };
            let Some(diag) = config.apply_rules(DiagnosticSource::Semantic, diag) else {
                ignored_semantic_diagnostic_count += 1;
                return None;
            };
            Some(CompilationDiagnostic {
                file_id: diag_file_id,
                source: DiagnosticSource::Semantic,
                diagnostic: diag,
            })
        }));
    }
    tracing::info!(
        raw_semantic_diagnostic_count,
        unmapped_semantic_buffer_count,
        ignored_semantic_diagnostic_count,
        diagnostic_count = diagnostics.len(),
        "semantic diagnostics complete"
    );

    Arc::from(diagnostics)
}

fn source_root_semantic_diagnostics(
    db: &dyn SourceRootDb,
    file_id: FileId,
) -> Arc<[(FileId, SyntaxDiagnostic)]> {
    Arc::from(
        db.file_compilation_diagnostics(file_id)
            .iter()
            .filter_map(|diag| {
                (diag.source == DiagnosticSource::Semantic)
                    .then_some((diag.file_id, diag.diagnostic.clone()))
            })
            .collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use ::preproc::source::{
        PreprocSourceId, SourceMacroExpansionId, SourcePreprocUnavailable, SourceRange,
    };
    use rustc_hash::FxHashSet;
    use syntax::{PreprocessorTrace, SourceBufferId, SourceBufferOrigin, SyntaxTreeOptions};
    use utils::{
        line_index::TextRange,
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{FileSet, VfsPath};

    use super::*;
    use crate::base_db::{
        project::{CompilationProfile, Predefine, PredefineSource},
        salsa::{self, Durability},
    };

    const TOP: FileId = FileId(0);
    const MANIFEST: FileId = FileId(1);
    const ROOT: SourceRootId = SourceRootId(0);

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

    fn db_with_root_file() -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
        let mut files = FxHashSet::default();
        files.insert(TOP);

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(TOP, ROOT, Durability::LOW);
        db.set_file_path_with_durability(TOP, Some(top_path), Durability::LOW);
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(
            TOP,
            Arc::from("module top; endmodule\n"),
            Durability::LOW,
        );
        db
    }

    fn db_with_root_and_manifest(manifest_text: &str) -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let manifest_path = abs_path("vide.toml");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        file_set.insert(MANIFEST, VfsPath::from(manifest_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);
        let mut files = FxHashSet::default();
        files.insert(TOP);
        files.insert(MANIFEST);

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        for (file_id, path, kind, text) in [
            (TOP, top_path, SourceFileKind::SystemVerilog, "module top; endmodule\n"),
            (MANIFEST, manifest_path, SourceFileKind::ProjectManifest, manifest_text),
        ] {
            db.set_source_root_id_with_durability(file_id, ROOT, Durability::LOW);
            db.set_file_path_with_durability(file_id, Some(path), Durability::LOW);
            db.set_file_kind_with_durability(file_id, kind, Durability::LOW);
            db.set_file_text_with_durability(file_id, Arc::from(text), Durability::LOW);
        }
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::try_from(text.find(needle).expect("needle must exist")).unwrap()
    }

    fn offset_after(text: &str, needle: &str) -> TextSize {
        offset(text, needle) + TextSize::try_from(needle.len()).unwrap()
    }

    #[test]
    fn include_headers_are_not_standalone_parse_diagnostic_units() {
        let kind =
            SourceFileKind::from_path(&VfsPath::new_virtual_path("/include/defs.svh".into()));

        assert_eq!(kind, SourceFileKind::IncludeHeader);
        assert!(!kind.is_slang_parse_unit());
    }

    #[test]
    fn systemverilog_sources_remain_parse_diagnostic_units() {
        let kind = SourceFileKind::from_path(&VfsPath::new_virtual_path("/rtl/top.sv".into()));

        assert_eq!(kind, SourceFileKind::SystemVerilog);
        assert!(kind.is_slang_parse_unit());
    }

    #[test]
    fn project_manifests_are_not_slang_parse_diagnostic_units() {
        let kind = SourceFileKind::from_path(&VfsPath::new_virtual_path("/root/vide.toml".into()));

        assert_eq!(kind, SourceFileKind::ProjectManifest);
        assert!(!kind.is_slang_parse_unit());
    }

    #[test]
    fn project_manifests_are_loadable_but_not_semantic_or_preproc_inputs() {
        let top_path = abs_path("rtl/top.sv");
        let manifest_path = abs_path("vide.toml");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        file_set.insert(MANIFEST, VfsPath::from(manifest_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP]);

        let mut files = FxHashSet::default();
        files.insert(TOP);
        files.insert(MANIFEST);

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        for (file_id, path, kind, text) in [
            (TOP, top_path, SourceFileKind::SystemVerilog, "module top; endmodule\n"),
            (MANIFEST, manifest_path, SourceFileKind::ProjectManifest, "defines = [\"M=1\"]\n"),
        ] {
            db.set_source_root_id_with_durability(file_id, ROOT, Durability::LOW);
            db.set_file_path_with_durability(file_id, Some(path), Durability::LOW);
            db.set_file_kind_with_durability(file_id, kind, Durability::LOW);
            db.set_file_text_with_durability(file_id, Arc::from(text), Durability::LOW);
        }
        db.set_project_config_with_durability(
            Arc::new(ProjectConfig::new(
                vec![Some(CompilationProfileId(0))],
                vec![CompilationProfile {
                    source_roots: vec![ROOT],
                    top_modules: Vec::new(),
                    preprocess: PreprocessConfig::default(),
                }],
            )),
            Durability::LOW,
        );

        assert_eq!(db.file_kind(MANIFEST), SourceFileKind::ProjectManifest);
        assert!(db.parse_diagnostics(MANIFEST).is_empty());

        let plan = db.compilation_plan_for_root(ROOT);
        assert_eq!(plan.roots, vec![TOP]);
        assert!(!plan.include_only.contains(&MANIFEST));

        let preproc_model_files =
            workspace_preproc_model_file_ids(&db, Some(CompilationProfileId(0)));
        assert_eq!(preproc_model_files, vec![TOP]);
        assert_eq!(
            db.source_preproc_model(MANIFEST).as_ref(),
            &Err(SourcePreprocQueryError::UnsupportedFileKind(SourceFileKind::ProjectManifest))
        );
    }

    #[test]
    fn source_preproc_mapping_reports_unmapped_included_source() {
        let db = db_with_root_file();
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    text: None,
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: abs_path("include/missing.vh").to_string(),
                    text: None,
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Source,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions::default();
        let preprocess = PreprocessConfig::default();
        let source_map =
            source_preproc_file_ids(&db, TOP, None, &trace, &options, &preprocess).unwrap();

        assert_eq!(
            source_map.get(PreprocSourceId::from(2)),
            Some(&PreprocSourceMapping::Unmapped(SourcePreprocUnavailable::DetachedSource {
                source: PreprocSourceId::from(2),
            }))
        );
        assert!(matches!(
            source_map.file_id(PreprocSourceId::from(2)),
            Err(PreprocSourceMapError::UnmappedSource { .. })
        ));
    }

    #[test]
    fn source_preproc_mapping_records_predefines_by_verified_source_text() {
        let db = db_with_root_file();
        let first_text = materialized_predefine_text("FIRST=1");
        let second_text = materialized_predefine_text("SECOND");
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    text: None,
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(second_text.clone()),
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Predefine,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(first_text.clone()),
                    buffer_id: 9,
                    origin: SourceBufferOrigin::Predefine,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(materialized_predefine_text("EXTRA=9")),
                    buffer_id: 4,
                    origin: SourceBufferOrigin::Predefine,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            predefines: vec!["FIRST=1".to_owned(), "SECOND".to_owned()],
            ..SyntaxTreeOptions::default()
        };
        let preprocess =
            PreprocessConfig::with_predefine_strings(["FIRST=1", "SECOND"], Vec::new());

        let source_map =
            source_preproc_file_ids(&db, TOP, None, &trace, &options, &preprocess).unwrap();
        let first = PreprocSourceId::from(9);
        let second = PreprocSourceId::from(2);
        let extra = PreprocSourceId::from(4);
        let expected_path = preproc_virtual_predefines_path(None);

        let Some(PreprocSourceMapping::VirtualDisplay { path, origin }) = source_map.get(first)
        else {
            panic!("first predefine should map to display-only virtual source");
        };
        assert_eq!(path, &expected_path);
        assert_eq!(origin, &PreprocVirtualOrigin::Predefines { profile: None });

        assert_eq!(
            source_map.get(second),
            Some(&PreprocSourceMapping::VirtualDisplay {
                path: expected_path,
                origin: PreprocVirtualOrigin::Predefines { profile: None },
            })
        );
        assert_eq!(
            source_map.get(extra),
            Some(&PreprocSourceMapping::Unmapped(
                SourcePreprocUnavailable::UnverifiedPredefineSource { source: extra }
            ))
        );
        assert!(matches!(
            source_map.file_id(first),
            Err(PreprocSourceMapError::DisplayOnlyVirtualSource { .. })
        ));

        let second_range = SourceRange {
            source: second,
            range: TextRange::new(TextSize::from(0), TextSize::from(7)),
        };
        assert_eq!(
            source_map.map_range(second_range).unwrap(),
            TextRange::new(
                TextSize::from(u32::try_from(first_text.len()).unwrap()),
                TextSize::from(u32::try_from(first_text.len() + 7).unwrap()),
            )
        );
    }

    #[test]
    fn source_preproc_mapping_records_duplicate_predefine_occurrences() {
        let manifest_text = "defines = [\"FOO\", \"FOO=1\"]\n";
        let first_start = manifest_text.find("\"FOO\"").unwrap();
        let second_start = manifest_text.find("\"FOO=1\"").unwrap();
        let first_range = TextRange::new(
            TextSize::from(u32::try_from(first_start).unwrap()),
            TextSize::from(u32::try_from(first_start + "\"FOO\"".len()).unwrap()),
        );
        let second_range = TextRange::new(
            TextSize::from(u32::try_from(second_start).unwrap()),
            TextSize::from(u32::try_from(second_start + "\"FOO=1\"".len()).unwrap()),
        );
        let db = db_with_root_and_manifest(manifest_text);
        let predefine_text = materialized_predefine_text("FOO");
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    text: None,
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(predefine_text.clone()),
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Predefine,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(predefine_text.clone()),
                    buffer_id: 3,
                    origin: SourceBufferOrigin::Predefine,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            predefines: vec!["FOO".to_owned(), "FOO=1".to_owned()],
            ..SyntaxTreeOptions::default()
        };
        let preprocess = PreprocessConfig {
            predefines: vec![
                Predefine::with_source(
                    "FOO",
                    PredefineSource { path: abs_path("vide.toml"), range: first_range },
                ),
                Predefine::with_source(
                    "FOO=1",
                    PredefineSource { path: abs_path("vide.toml"), range: second_range },
                ),
            ],
            include_dirs: Vec::new(),
        };

        let source_map =
            source_preproc_file_ids(&db, TOP, None, &trace, &options, &preprocess).unwrap();
        let first = PreprocSourceId::from(2);
        let second = PreprocSourceId::from(3);

        assert!(matches!(source_map.get(first), Some(PreprocSourceMapping::VirtualDisplay { .. })));
        assert!(matches!(
            source_map.get(second),
            Some(PreprocSourceMapping::VirtualDisplay { .. })
        ));
        assert_eq!(source_map.predefine_manifest_source(first).unwrap().range, first_range);
        assert_eq!(source_map.predefine_manifest_source(second).unwrap().range, second_range);
        assert_eq!(
            source_map.map_range(SourceRange {
                source: first,
                range: TextRange::new(TextSize::from(0), TextSize::from(1)),
            }),
            Ok(TextRange::new(TextSize::from(0), TextSize::from(1)))
        );
        assert_eq!(
            source_map.map_range(SourceRange {
                source: second,
                range: TextRange::new(TextSize::from(0), TextSize::from(1)),
            }),
            Ok(TextRange::new(
                TextSize::from(u32::try_from(predefine_text.len()).unwrap()),
                TextSize::from(u32::try_from(predefine_text.len() + 1).unwrap()),
            ))
        );
    }

    #[test]
    fn source_preproc_mapping_rejects_predefine_source_text_mismatch() {
        let db = db_with_root_file();
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    text: None,
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(materialized_predefine_text("SECOND=2")),
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Predefine,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            predefines: vec!["FIRST=1".to_owned()],
            ..SyntaxTreeOptions::default()
        };
        let preprocess = PreprocessConfig::with_predefine_strings(["FIRST=1"], Vec::new());

        let source_map =
            source_preproc_file_ids(&db, TOP, None, &trace, &options, &preprocess).unwrap();
        let source = PreprocSourceId::from(2);

        assert_eq!(
            source_map.get(source),
            Some(&PreprocSourceMapping::Unmapped(
                SourcePreprocUnavailable::UnverifiedPredefineSource { source }
            ))
        );
        assert!(matches!(
            source_map.map_range(SourceRange {
                source,
                range: TextRange::new(TextSize::from(0), TextSize::from(1)),
            }),
            Err(PreprocSourceMapError::UnmappedSource { .. })
        ));
    }

    #[test]
    fn source_preproc_mapping_rejects_manifest_range_mismatch() {
        let manifest_text = "defines = [\"RIGHT=1\", \"WRONG=2\"]\n";
        let db = db_with_root_and_manifest(manifest_text);
        let wrong_range = TextRange::new(
            offset(manifest_text, "\"WRONG=2\""),
            offset_after(manifest_text, "\"WRONG=2\""),
        );
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    text: None,
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: "<api>".to_owned(),
                    text: Some(materialized_predefine_text("RIGHT=1")),
                    buffer_id: 2,
                    origin: SourceBufferOrigin::Predefine,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            predefines: vec!["RIGHT=1".to_owned()],
            ..SyntaxTreeOptions::default()
        };
        let preprocess = PreprocessConfig {
            predefines: vec![Predefine::with_source(
                "RIGHT=1",
                PredefineSource { path: abs_path("vide.toml"), range: wrong_range },
            )],
            include_dirs: Vec::new(),
        };

        let source_map =
            source_preproc_file_ids(&db, TOP, None, &trace, &options, &preprocess).unwrap();
        let source = PreprocSourceId::from(2);

        assert_eq!(
            source_map.get(source),
            Some(&PreprocSourceMapping::Unmapped(
                SourcePreprocUnavailable::UnverifiedPredefineSource { source }
            ))
        );
    }

    #[test]
    fn source_preproc_mapping_records_external_include_buffer_as_display_virtual_source() {
        let db = db_with_root_file();
        let external_path = "/external/generated_defs.vh".to_owned();
        let trace = PreprocessorTrace {
            root_buffer_id: 1,
            source_buffers: vec![
                SourceBufferId {
                    path: abs_path("rtl/top.v").to_string(),
                    text: None,
                    buffer_id: 1,
                    origin: SourceBufferOrigin::Source,
                },
                SourceBufferId {
                    path: external_path.clone(),
                    text: None,
                    buffer_id: 4,
                    origin: SourceBufferOrigin::Source,
                },
            ],
            events: Vec::new(),
            include_edges: Vec::new(),
            emitted_tokens: Vec::new(),
        };
        let options = SyntaxTreeOptions {
            include_buffers: vec![SyntaxTreeBuffer {
                path: external_path,
                text: "`define FROM_BUFFER 1\n".to_owned(),
            }],
            ..SyntaxTreeOptions::default()
        };

        let preprocess = PreprocessConfig::default();
        let source_map = source_preproc_file_ids(
            &db,
            TOP,
            Some(CompilationProfileId(7)),
            &trace,
            &options,
            &preprocess,
        )
        .unwrap();
        let source = PreprocSourceId::from(4);
        let Some(PreprocSourceMapping::VirtualDisplay { path, origin }) = source_map.get(source)
        else {
            panic!("external include buffer should map to display-only virtual source");
        };

        assert_eq!(
            path,
            &VfsPath::new_virtual_path(
                "/__vide/preproc/profile-7/include-buffer/4/generated_defs.svh".to_owned()
            )
        );
        assert_eq!(origin, &PreprocVirtualOrigin::ExternalIncludeBuffer { source });
        assert!(matches!(
            source_map.map_range(SourceRange {
                source,
                range: TextRange::new(TextSize::from(0), TextSize::from(128)),
            }),
            Err(PreprocSourceMapError::RangeOutOfBounds { .. })
        ));
    }

    #[test]
    fn preproc_virtual_paths_use_reserved_namespace() {
        assert_eq!(
            preproc_virtual_predefines_path(None),
            VfsPath::new_virtual_path("/__vide/preproc/default/predefines.sv".to_owned())
        );
        assert_eq!(
            preproc_virtual_builtin_path(Some(CompilationProfileId(3)), "bad/name"),
            VfsPath::new_virtual_path("/__vide/preproc/profile-3/builtin/bad_name.sv".to_owned())
        );
        assert_eq!(
            preproc_virtual_expansion_path(
                Some(CompilationProfileId(3)),
                SourceMacroExpansionId::new(9),
            ),
            VfsPath::new_virtual_path("/__vide/preproc/profile-3/expansion/9.sv".to_owned())
        );
        assert_eq!(
            preproc_virtual_speculative_path(
                Some(CompilationProfileId(3)),
                PreprocSpeculativeUniverseId(11),
                "root/top",
            ),
            VfsPath::new_virtual_path(
                "/__vide/preproc/profile-3/speculative/11/root_top.sv".to_owned()
            )
        );
    }
}
