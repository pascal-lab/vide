use std::hash::{Hash, Hasher};

use base_db::{
    analysis_snapshot::CompilationContext,
    diagnostics_config::{DiagnosticSource, DiagnosticsConfig},
    project::CompilationProfileId,
    source_db::{SourceFileKind, SourceRootDb},
    source_root::SourceRootId,
};
use rustc_hash::FxHasher;
use syntax::{
    SyntaxTree, SyntaxTreeBuffer,
    diagnostics::{ParserExpectedSyntax, SyntaxDiagnostic},
    preproc::Trace,
};
use triomphe::Arc;
use utils::{line_index::TextSize, path_identity::PathIdentityIndex};
use vfs::FileId;

use crate::{
    compilation_plan::{self, CompilationPlan},
    context::{MacroCoverage, file_macro_coverage_query},
    file::HirFileId,
    macro_file::{self, ExpandResult, ExpansionInfo, MacroFileId, TraceIndex},
    preproc::{MacroReferenceIndex, macro_reference_index_for_profile_query},
    source_db::{
        MappedSourcePreprocModel, SourcePreprocContextIndex, SourcePreprocQueryError,
        SourcePreprocRelevantContexts, source_preproc_context_index_for_profile,
        source_preproc_contexts_for_file, source_preproc_model,
    },
};

struct SourceFileIdentity {
    name: String,
    path: String,
}

// Salsa 0.28 tracked functions require salsa-struct arguments. `FileId` and
// `Option<CompilationProfileId>` are plain integers, so they need interned
// wrappers to serve as tracked-function keys. Untracked functions accept the
// direct types — these wrappers are only used at the tracked-function boundary.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct PreprocFileQueryKey {
    #[returns(copy)]
    pub file_id: FileId,
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct PreprocProfileQueryKey {
    #[returns(copy)]
    pub profile_id: Option<CompilationProfileId>,
}

/// Singleton key for the workspace-global path index (one per database).
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct WorkspacePathIndexKey {
    #[returns(copy)]
    pub unit: (),
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
    pub preprocessor_trace: Option<Trace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompilationUnitId {
    pub root_file: FileId,
    pub profile: Option<CompilationProfileId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationDependencyManifest {
    pub files: Arc<[FileId]>,
}

/// Immutable identity of every input that can affect one standalone Slang
/// parse. The fingerprint is diagnostic; Salsa keys the compiler artifact by a
/// tracked input containing the complete value, so hash collisions cannot
/// alias compiler artifacts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationUnitSnapshot {
    pub id: CompilationUnitId,
    pub fingerprint: u64,
    pub dependencies: CompilationDependencyManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CompilationUnitInputs {
    id: CompilationUnitId,
    kind: SourceFileKind,
    name: String,
    path: String,
    text: Arc<str>,
    options: Arc<syntax::SyntaxTreeOptions>,
    dependencies: Arc<[FileId]>,
}

#[salsa::tracked(debug)]
struct CompilationUnitArtifactInput<'db> {
    #[returns(clone)]
    inputs: Arc<CompilationUnitInputs>,
}

/// A strictly single-file source model for editor-local operations.
///
/// Unlike [`ParsedCompilationUnit`], this model never expands includes or
/// reads profile predefines. Its complete dependency set is the file text,
/// file kind, and display identity, so edits elsewhere cannot invalidate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceModel {
    pub syntax_tree: SyntaxTree,
    pub preprocessor_independent: bool,
}

fn source_file_identity(db: &dyn SourceRootDb, file_id: FileId) -> SourceFileIdentity {
    let path = compilation_plan::source_buffer_path(db, file_id).to_string();
    let name =
        db.file_path(file_id).map(|path| path.to_string()).unwrap_or_else(|| "source".to_owned());
    SourceFileIdentity { name, path }
}

#[salsa::tracked(lru = 128, returns(clone))]
fn source_model(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Arc<SourceModel> {
    let file_id = key.file_id(db);
    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let syntax_tree = match db.file_kind(file_id) {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            SyntaxTree::from_file_in_memory_with_options(
                &text,
                &identity.name,
                &identity.path,
                &syntax::SyntaxTreeOptions::without_include_expansion(),
            )
        }
        SourceFileKind::LibraryMap => {
            SyntaxTree::from_library_map_text(&text, &identity.name, &identity.path)
        }
        SourceFileKind::ProjectManifest => SyntaxTree::from_text("", "", ""),
    };
    let trace = syntax_tree.preprocessor_trace();
    let preprocessor_independent = trace.events.is_empty()
        && trace.include_edges.is_empty()
        && trace
            .emitted_tokens
            .iter()
            .all(|token| matches!(token.origin, syntax::preproc::TokenOrigin::Source { .. }));
    Arc::new(SourceModel { syntax_tree, preprocessor_independent })
}

/// Workspace-global path-spelling → [`FileId`] index, memoized per revision.
#[salsa::tracked(returns(clone))]
fn path_file_ids(db: &dyn PreprocDb, _key: WorkspacePathIndexKey) -> PathIdentityIndex<FileId> {
    let mut index = PathIdentityIndex::default();
    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }
        let path = compilation_plan::source_buffer_path(db, file_id);
        index.insert_path(&path, file_id);
    }
    index
}

pub(crate) fn syntax_tree_options_for_file(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    let _span = tracing::info_span!("slang.syntax_tree_options.file", ?file_id).entered();
    let profile_id = db.file_compilation_profile(file_id);
    let preprocess = db.project_config().preprocess_for_profile(profile_id);
    let identity = source_file_identity(db, file_id);
    let include_buffers = compilation_plan::include_buffers_for_file(db, file_id)
        .into_iter()
        .filter(|buffer| buffer.path != identity.path)
        .collect();
    syntax::SyntaxTreeOptions {
        predefines: preprocess.predefine_strings(),
        include_paths: preprocess.include_dir_strings(),
        include_buffers,
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn syntax_tree_options_for_parser_cursor(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    syntax_tree_options_for_file(db, file_id)
}

#[salsa::tracked(lru = 128, returns(clone))]
fn compilation_unit_inputs(
    db: &dyn PreprocDb,
    key: PreprocFileQueryKey,
) -> Arc<CompilationUnitInputs> {
    let file_id = key.file_id(db);
    let profile_id = db.file_compilation_profile(file_id);
    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let kind = db.file_kind(file_id);
    let options = match kind {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            // Profile predefines + this file's static include closure.
            // Predecessor `$unit` macros are not injected here: that walk
            // builds the profile include plan and re-parses every earlier
            // root. This file's own includes carry the macros it uses.
            syntax_tree_options_for_file(db, file_id)
        }
        SourceFileKind::LibraryMap | SourceFileKind::ProjectManifest => {
            syntax::SyntaxTreeOptions::default()
        }
    };
    let mut dependencies = vec![file_id];
    let path_file_ids = db.path_file_ids();
    dependencies.extend(
        options.include_buffers.iter().filter_map(|buffer| path_file_ids.get(&buffer.path)),
    );
    dependencies.sort_unstable_by_key(|dependency| dependency.index());
    dependencies.dedup();
    Arc::new(CompilationUnitInputs {
        id: CompilationUnitId { root_file: file_id, profile: profile_id },
        kind,
        name: identity.name,
        path: identity.path,
        text,
        options: Arc::new(options),
        dependencies: Arc::from(dependencies),
    })
}

#[salsa::tracked(lru = 128, returns(clone))]
fn compilation_unit_snapshot(
    db: &dyn PreprocDb,
    key: PreprocFileQueryKey,
) -> Arc<CompilationUnitSnapshot> {
    let inputs = compilation_unit_inputs(db, key);
    let mut hasher = FxHasher::default();
    inputs.hash(&mut hasher);
    let fingerprint = hasher.finish();
    Arc::new(CompilationUnitSnapshot {
        id: inputs.id,
        fingerprint,
        dependencies: CompilationDependencyManifest { files: inputs.dependencies.clone() },
    })
}

#[salsa::tracked]
fn compilation_unit_artifact_input<'db>(
    db: &'db dyn PreprocDb,
    key: PreprocFileQueryKey,
) -> CompilationUnitArtifactInput<'db> {
    CompilationUnitArtifactInput::new(db, compilation_unit_inputs(db, key))
}

/// Content-addressed Slang artifact store. Salsa interns the complete immutable
/// input value and memoizes this parse by that identity across revisions.
#[salsa::tracked(lru = 128, returns(clone))]
fn compilation_unit_artifact(
    db: &dyn PreprocDb,
    key: CompilationUnitArtifactInput<'_>,
) -> Arc<ParsedCompilationUnit> {
    let inputs = key.inputs(db);
    let _span = tracing::info_span!(
        "slang.compilation_unit_artifact",
        file_id = ?inputs.id.root_file,
        profile_id = ?inputs.id.profile,
        include_buffer_count = inputs.options.include_buffers.len(),
        bytes = inputs.text.len(),
    )
    .entered();
    let (syntax_tree, preprocessor_trace) = match inputs.kind {
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader => {
            let parsed = SyntaxTree::from_file_in_memory_with_options_and_trace(
                &inputs.text,
                &inputs.name,
                &inputs.path,
                &inputs.options,
            );
            (parsed.tree, Some(parsed.preprocessor_trace))
        }
        SourceFileKind::LibraryMap => {
            (SyntaxTree::from_library_map_text(&inputs.text, &inputs.name, &inputs.path), None)
        }
        SourceFileKind::ProjectManifest => (SyntaxTree::from_text("", "", ""), None),
    };
    Arc::new(ParsedCompilationUnit { syntax_tree, preprocessor_trace })
}

fn parse_tree(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> SyntaxTree {
    let input = compilation_unit_artifact_input(db, key);
    compilation_unit_artifact(db, *input).syntax_tree.clone()
}

/// Preprocessor trace of one file, split from [`parse_tree`] so a syntax-only
/// edit (e.g. a comment) re-parses the tree without invalidating the trace or
/// the downstream preprocessor model and `$unit` macro chain.
#[salsa::tracked(lru = 128, returns(clone))]
fn preproc_trace(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Option<Trace> {
    let input = compilation_unit_artifact_input(db, key);
    compilation_unit_artifact(db, *input).preprocessor_trace.clone()
}

/// Files actually consumed by one authoritative standalone parse.
///
/// The preprocessor's emitted include edges are the dependency identity. This
/// deliberately does not infer reverse dependencies from source text or from
/// the profile-wide include plan.
fn parsed_compilation_dependencies(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Arc<[FileId]> {
    let file_id = key.file_id(db);
    let input = compilation_unit_artifact_input(db, key);
    let parsed = compilation_unit_artifact(db, *input);
    dependencies_from_parsed_compilation(db, file_id, &parsed)
}

fn dependencies_from_parsed_compilation(
    db: &dyn PreprocDb,
    file_id: FileId,
    parsed: &ParsedCompilationUnit,
) -> Arc<[FileId]> {
    let mut dependencies = vec![file_id];
    if let Some(trace) = &parsed.preprocessor_trace {
        let path_file_ids = db.path_file_ids();
        dependencies.extend(trace.include_edges.iter().filter_map(|edge| {
            let buffer = trace
                .source_buffers
                .iter()
                .find(|buffer| buffer.buffer_id == edge.included_buffer_id)?;
            path_file_ids.get(&buffer.path)
        }));
    }
    dependencies.sort_unstable_by_key(|dependency| dependency.index());
    dependencies.dedup();
    Arc::from(dependencies)
}

/// `define` directives this file contributes to the compilation-unit scope,
/// reconstructed verbatim so they can be injected as predefines into later
/// roots' standalone parses. Include-derived macros are excluded: each root
/// re-processes its own includes.
#[salsa::tracked(returns(clone))]
fn unit_macro_contribution(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Arc<[String]> {
    let file_id = key.file_id(db);
    let model = db.source_preproc_model(file_id);
    let Ok(model) = model.as_ref() else {
        return Arc::from(Vec::<String>::new());
    };
    let text = db.file_text(file_id);
    let mut defines = Vec::new();
    for def in model.model.macro_definitions().iter() {
        if model.source_map.file_id(def.directive_range.source).ok() != Some(file_id) {
            continue;
        }
        let start = usize::from(def.directive_range.range.start());
        let end = usize::from(def.directive_range.range.end());
        if let Some(raw) = text.get(start..end) {
            defines.push(raw.to_string());
        }
    }
    Arc::from(defines)
}

/// Running compilation-unit macro set of every root before `file_id`, in
/// compilation order. Injected as predefines so a standalone parse sees the
/// same `$unit` macros the monolithic profile parse would.
#[salsa::tracked(returns(clone))]
fn unit_macro_predefines(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Arc<[String]> {
    let file_id = key.file_id(db);
    let profile_id = db.file_compilation_profile(file_id);
    let plan = db.compilation_plan_for_profile(profile_id);
    let mut predefines = Vec::new();
    for &root in &plan.roots {
        if root == file_id {
            break;
        }
        predefines.extend(
            unit_macro_contribution(db, PreprocFileQueryKey::new(db, root)).iter().cloned(),
        );
    }
    Arc::from(predefines)
}

#[salsa::tracked(lru = 128, returns(clone))]
fn parse_src_for_compilation(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> SyntaxTree {
    let file_id = key.file_id(db);
    db.parse_tree(file_id)
}

pub fn set_parse_lru_capacity(db: &mut dyn PreprocDb, capacity: usize) {
    parse_src_for_compilation::set_lru_capacity(db, capacity);
    compilation_unit_inputs::set_lru_capacity(db, capacity);
    compilation_unit_snapshot::set_lru_capacity(db, capacity);
    compilation_unit_artifact::set_lru_capacity(db, capacity);
    preproc_trace::set_lru_capacity(db, capacity);
    crate::source_db::set_source_preproc_model_lru_capacity(db, capacity);
    crate::macro_file::set_macro_expansion_lru_capacity(db, capacity);
    crate::macro_file::set_trace_index_lru_capacity(db, capacity);
}

/// Parser expectations at one cursor offset.
///
/// Expectations are completion-only metadata. Keep them out of authoritative
/// compilation trees and ask Slang for a cursor-scoped parse when completion
/// needs them. The fallback receives the same profile buffers and options as
/// the authoritative parse, so an unsaved include cannot silently come from
/// disk.
#[salsa::tracked(lru = 64, returns(clone))]
fn parser_expected_syntax(
    db: &dyn PreprocDb,
    key: PreprocFileQueryKey,
    offset: TextSize,
) -> Arc<[ParserExpectedSyntax]> {
    let file_id = key.file_id(db);
    if matches!(db.file_kind(file_id), SourceFileKind::ProjectManifest) {
        return Arc::from(Vec::<ParserExpectedSyntax>::new());
    }

    if matches!(db.file_kind(file_id), SourceFileKind::LibraryMap) {
        // Library maps are tiny and parsed without expectation collection;
        // keep the dedicated single-cursor parse for them.
        let text = db.file_text(file_id);
        let identity = source_file_identity(db, file_id);
        let offset = usize::from(offset);
        return Arc::from(SyntaxTree::library_map_expected_syntax_at_offset(
            &text,
            &identity.name,
            &identity.path,
            offset,
        ));
    }

    let text = db.file_text(file_id);
    let identity = source_file_identity(db, file_id);
    let options = syntax_tree_options_for_parser_cursor(db, file_id);
    Arc::from(SyntaxTree::expected_syntax_at_offset_with_options(
        &text,
        &identity.name,
        &identity.path,
        usize::from(offset),
        &options,
    ))
}

fn slang_warning_options(config: &DiagnosticsConfig) -> Vec<String> {
    match &config.slang.warnings {
        Some(options) if options.is_empty() => vec!["none".to_owned()],
        Some(options) => options.clone(),
        None => Vec::new(),
    }
}

fn parse_diagnostics(db: &dyn PreprocDb, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
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
        tree.diagnostics_with_options(&slang_warning_options(&config))
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

/// Derived preprocessing, compilation, and macro-expansion queries.
///
/// This is the first semantic database layer. Keeping the interface in this
/// crate makes a `base-db -> preproc-expand` dependency impossible in Cargo.
#[salsa::db]
pub trait PreprocDb: SourceRootDb {}

impl dyn PreprocDb + '_ {
    pub fn compilation_plan_for_root(&self, source_root_id: SourceRootId) -> Arc<CompilationPlan> {
        compilation_plan_for_root(self, source_root_id)
    }

    pub fn compilation_plan_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationPlan> {
        compilation_plan_for_profile(self, PreprocProfileQueryKey::new(self, profile_id))
    }

    pub fn static_include_closure(
        &self,
        file_id: FileId,
    ) -> compilation_plan::StaticIncludeClosure {
        compilation_plan::static_include_closure(self, file_id)
    }

    pub fn compilation_context(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationContext> {
        compilation_context(self, profile_id)
    }

    pub fn compilation_context_for_file(&self, file_id: FileId) -> Arc<CompilationContext> {
        compilation_context_for_file(self, file_id)
    }

    pub fn include_buffers_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<Vec<SyntaxTreeBuffer>> {
        include_buffers_for_profile(self, profile_id)
    }

    pub fn source_preproc_model(
        &self,
        file_id: FileId,
    ) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>> {
        source_preproc_model(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn source_preproc_context_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<SourcePreprocContextIndex> {
        source_preproc_context_index_for_profile(self, profile_id)
    }

    pub fn source_preproc_contexts_for_file(
        &self,
        file_id: FileId,
    ) -> Arc<SourcePreprocRelevantContexts> {
        source_preproc_contexts_for_file(self, file_id)
    }

    pub fn parse_tree(&self, file_id: FileId) -> SyntaxTree {
        parse_tree(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn compilation_unit_snapshot(&self, file_id: FileId) -> Arc<CompilationUnitSnapshot> {
        compilation_unit_snapshot(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn source_model(&self, file_id: FileId) -> Arc<SourceModel> {
        source_model(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn preproc_trace(&self, file_id: FileId) -> Option<Trace> {
        preproc_trace(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn parsed_compilation_dependencies(&self, file_id: FileId) -> Arc<[FileId]> {
        parsed_compilation_dependencies(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn parse_src_with_dependencies(&self, file_id: FileId) -> (SyntaxTree, Arc<[FileId]>) {
        let key = PreprocFileQueryKey::new(self, file_id);
        let input = compilation_unit_artifact_input(self, key);
        let parsed = compilation_unit_artifact(self, *input);
        let dependencies = dependencies_from_parsed_compilation(self, file_id, &parsed);
        (parsed.syntax_tree.clone(), dependencies)
    }

    pub fn unit_macro_predefines(&self, file_id: FileId) -> Arc<[String]> {
        unit_macro_predefines(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn path_file_ids(&self) -> PathIdentityIndex<FileId> {
        path_file_ids(self, WorkspacePathIndexKey::new(self, ()))
    }

    pub fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree {
        parse_src_for_compilation(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Arc<[ParserExpectedSyntax]> {
        parser_expected_syntax(self, PreprocFileQueryKey::new(self, file_id), offset)
    }

    pub fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
        parse_diagnostics(self, file_id)
    }

    pub fn macro_expansion(&self, macro_file: MacroFileId) -> Arc<ExpandResult<ExpansionInfo>> {
        macro_file::macro_expansion_query(self, macro_file)
    }

    pub fn parse(&self, file_id: HirFileId) -> SyntaxTree {
        parse(self, file_id)
    }

    pub fn trace_index(&self, model_file: FileId) -> Arc<TraceIndex> {
        macro_file::trace_index_query(self, PreprocFileQueryKey::new(self, model_file))
    }

    pub fn file_macro_coverage(&self, file_id: FileId) -> Arc<MacroCoverage> {
        file_macro_coverage_query(self, file_id)
    }

    pub fn source_semantic_map(&self, file_id: FileId) -> Arc<macro_file::SourceSemanticMap> {
        macro_file::source_semantic_map_query(self, PreprocFileQueryKey::new(self, file_id))
    }

    pub fn macro_reference_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<MacroReferenceIndex> {
        macro_reference_index_for_profile_query(self, PreprocProfileQueryKey::new(self, profile_id))
    }
}

fn parse(db: &dyn PreprocDb, file_id: HirFileId) -> SyntaxTree {
    match file_id {
        HirFileId::File(file_id) => db.parse_src_for_compilation(file_id),
        HirFileId::Macro(macro_file) => {
            let expansion = db.macro_expansion(macro_file);
            if let Some(error) = &expansion.err {
                tracing::warn!(
                    ?macro_file,
                    ?error,
                    "macro HIR parse is based on a partial expansion"
                );
            }
            expansion.value.parse.clone()
        }
    }
}

fn compilation_plan_for_root(
    db: &dyn PreprocDb,
    source_root_id: SourceRootId,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_source_root(db, source_root_id))
}

#[salsa::tracked(lru = 32, returns(clone))]
fn compilation_plan_for_profile(
    db: &dyn PreprocDb,
    key: PreprocProfileQueryKey,
) -> Arc<CompilationPlan> {
    let profile_id = key.profile_id(db);
    Arc::new(CompilationPlan::for_profile(db, profile_id))
}

fn compilation_context(
    db: &dyn PreprocDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<CompilationContext> {
    let plan = db.compilation_plan_for_profile(profile_id);
    let library_maps = plan
        .roots
        .iter()
        .copied()
        .filter(|file_id| matches!(db.file_kind(*file_id), SourceFileKind::LibraryMap))
        .collect::<Vec<_>>();
    Arc::new(CompilationContext::new(
        profile_id,
        plan.roots.clone(),
        plan.include_dirs.clone(),
        plan.predefines.clone(),
        library_maps,
        plan.top_modules.clone(),
    ))
}

fn compilation_context_for_file(db: &dyn PreprocDb, file_id: FileId) -> Arc<CompilationContext> {
    let profile_id = db.file_compilation_profile(file_id);
    db.compilation_context(profile_id)
}

fn include_buffers_for_profile(
    db: &dyn PreprocDb,
    profile_id: Option<CompilationProfileId>,
) -> Arc<Vec<SyntaxTreeBuffer>> {
    let plan = db.compilation_plan_for_profile(profile_id);
    Arc::new(compilation_plan::include_buffers_for_plan(db, &plan))
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use ::preproc::source::{PreprocSourceId, SourcePreprocUnavailable, SourceRange};
    use base_db::{
        project::{
            CompilationProfile, Predefine, PredefineSource, PreprocessConfig, ProjectConfig,
        },
        salsa::{self, Durability},
        source_db::{FileLoader, SourceDb, SourceFileKind, SourceRootDb},
        source_root::SourceRoot,
    };
    use rustc_hash::FxHashSet;
    use syntax::{
        SyntaxTreeOptions,
        preproc::{SourceBufferId, SourceBufferOrigin, Trace},
    };
    use utils::{
        line_index::TextRange,
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{AnchoredPath, FileSet, VfsPath};

    use super::*;
    use crate::source_db::{
        PreprocSourceMapping, PreprocVirtualOrigin, SourcePreprocQueryError,
        materialized_predefine_text, preproc_virtual_predefines_path, source_preproc_file_ids,
        workspace_preproc_model_file_ids,
    };

    const TOP: FileId = FileId::from_raw(0);
    const MANIFEST: FileId = FileId::from_raw(1);
    const INCLUDED: FileId = FileId::from_raw(2);
    const ROOT: SourceRootId = SourceRootId(0);

    #[salsa::db]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    #[salsa::db]
    impl salsa::Database for TestDb {}

    #[salsa::db]
    impl SourceDb for TestDb {}

    #[salsa::db]
    impl SourceRootDb for TestDb {}

    #[salsa::db]
    impl PreprocDb for TestDb {}
    impl std::ops::Deref for TestDb {
        type Target = dyn PreprocDb;

        fn deref(&self) -> &Self::Target {
            self
        }
    }

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
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
        db.set_files_with_durability(files, Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(TOP, ROOT, Durability::LOW);
        db.set_file_kind_with_durability(TOP, SourceFileKind::SystemVerilog, Durability::LOW);
        db.set_file_text_with_durability(
            TOP,
            Arc::from("module top; endmodule\n"),
            Durability::LOW,
        );
        db
    }

    fn db_with_macro_included_root() -> TestDb {
        let top_path = abs_path("rtl/top.v");
        let included_path = abs_path("rtl/included.sv");
        let mut file_set = FileSet::default();
        file_set.insert(TOP, VfsPath::from(top_path.clone()));
        file_set.insert(INCLUDED, VfsPath::from(included_path.clone()));
        let root = SourceRoot::new_local_with_source_files(file_set, vec![TOP, INCLUDED]);
        let mut files = FxHashSet::default();
        files.insert(TOP);
        files.insert(INCLUDED);

        let mut db = TestDb::default();
        db.set_files_with_durability(files, Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        for (file_id, _path, text) in [
            (
                TOP,
                top_path,
                "`define FILE \"included.sv\"\n`include `FILE\nmodule top; endmodule\n",
            ),
            (INCLUDED, included_path, "module included; endmodule\n"),
        ] {
            db.set_source_root_id_with_durability(file_id, ROOT, Durability::LOW);
            db.set_file_kind_with_durability(
                file_id,
                SourceFileKind::SystemVerilog,
                Durability::LOW,
            );
            db.set_file_text_with_durability(file_id, Arc::from(text), Durability::LOW);
        }
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
        db.set_files_with_durability(files, Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        for (file_id, _path, kind, text) in [
            (TOP, top_path, SourceFileKind::SystemVerilog, "module top; endmodule\n"),
            (MANIFEST, manifest_path, SourceFileKind::ProjectManifest, manifest_text),
        ] {
            db.set_source_root_id_with_durability(file_id, ROOT, Durability::LOW);
            db.set_file_kind_with_durability(file_id, kind, Durability::LOW);
            db.set_file_text_with_durability(file_id, Arc::from(text), Durability::LOW);
        }
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:\\repo" } else { "/repo" };
        let sep = if cfg!(windows) { "\\" } else { "/" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}{sep}{path}")))
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
    fn source_model_never_expands_includes() {
        let db = db_with_macro_included_root();

        let source = db.source_model(TOP);
        let trace = source.syntax_tree.preprocessor_trace();

        assert!(trace.include_edges.is_empty());
        let included_path = abs_path("rtl/included.sv").to_string();
        assert!(trace.source_buffers.iter().all(|buffer| buffer.path != included_path));
    }

    #[test]
    fn systemverilog_sources_remain_parse_diagnostic_units() {
        let kind = SourceFileKind::from_path(&VfsPath::new_virtual_path("/rtl/top.sv".into()));

        assert_eq!(kind, SourceFileKind::SystemVerilog);
        assert!(kind.is_slang_parse_unit());
    }

    #[test]
    fn parser_expectations_are_cursor_scoped_outside_authoritative_tree() {
        let mut db = db_with_root_file();
        let text = "module top; always begin begin end endmodule\n";
        db.set_file_text_with_durability(TOP, Arc::from(text), Durability::LOW);
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::LOW);

        let tree = db.parse_tree(TOP);
        assert!(tree.expected_syntax_at(28).is_empty());
        assert!(!db.parser_expected_syntax(TOP, TextSize::from(28)).is_empty());
    }

    #[test]
    fn root_scoped_compilation_units_parse_standalone() {
        let mut db = db_with_root_file();
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::LOW);

        let compilation_tree = db.parse_tree(TOP);

        // Roots parse standalone now; the tree must still be a non-empty
        // compilation unit rather than sharing the profile's buffer identity.
        assert!(compilation_tree.root().children().next().is_some());
    }

    #[test]
    fn profile_compilation_units_parse_standalone() {
        let mut db = db_with_root_file();
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

        let compilation_tree = db.parse_tree(TOP);

        assert!(compilation_tree.root().children().next().is_some());
    }

    #[test]
    fn compilation_plan_updates_when_one_files_include_directives_change() {
        let mut db = db_with_macro_included_root();
        db.set_file_text_with_durability(
            TOP,
            Arc::from("`include \"included.sv\"\nmodule top; endmodule\n"),
            Durability::LOW,
        );

        let before = db.compilation_plan_for_profile(None);
        assert!(before.include_only.contains(&INCLUDED));
        assert!(!before.roots.contains(&INCLUDED));

        db.set_file_text_with_durability(
            TOP,
            Arc::from("module top; endmodule\n"),
            Durability::LOW,
        );

        let after = db.compilation_plan_for_profile(None);
        assert!(!after.include_only.contains(&INCLUDED));
        assert!(after.roots.contains(&INCLUDED));
    }

    #[test]
    fn compilation_plan_propagates_include_changes_to_includers() {
        let mut db = db_with_macro_included_root();
        db.set_file_text_with_durability(
            TOP,
            Arc::from("`include \"included.sv\"\nmodule top; endmodule\n"),
            Durability::LOW,
        );
        let plan = db.compilation_plan_for_profile(None);

        let affected = plan.affected_files([INCLUDED]);

        assert!(affected.contains(&INCLUDED));
        assert!(affected.contains(&TOP));
    }

    #[test]
    fn compilation_unit_fingerprint_covers_include_contents() {
        let mut db = db_with_macro_included_root();
        db.set_file_text_with_durability(
            TOP,
            Arc::from("`include \"included.sv\"\nmodule top; endmodule\n"),
            Durability::LOW,
        );
        let before = db.compilation_unit_snapshot(TOP);

        db.set_file_text_with_durability(
            INCLUDED,
            Arc::from("module included_changed; endmodule\n"),
            Durability::LOW,
        );
        let after = db.compilation_unit_snapshot(TOP);

        assert_ne!(before.fingerprint, after.fingerprint);
        assert!(after.dependencies.files.contains(&INCLUDED));
    }

    #[test]
    fn standalone_parse_registers_only_the_static_include_closure() {
        let mut db = db_with_macro_included_root();
        db.set_file_text_with_durability(
            TOP,
            Arc::from("`include \"included.sv\"\nmodule top; endmodule\n"),
            Durability::LOW,
        );

        let closure = db.static_include_closure(TOP);
        assert!(closure.is_complete(), "{closure:?}");
        assert_eq!(closure.files(), &[INCLUDED]);

        let options = syntax_tree_options_for_file(&db, TOP);
        assert_eq!(options.include_buffers.len(), 1);
        assert!(
            options.include_buffers[0].path.ends_with("included.sv"),
            "{}",
            options.include_buffers[0].path
        );
    }

    #[test]
    fn parsed_dependencies_follow_emitted_include_edges() {
        let mut db = db_with_macro_included_root();
        db.set_file_text_with_durability(
            TOP,
            Arc::from("`include \"included.sv\"\nmodule top; endmodule\n"),
            Durability::LOW,
        );

        let _ = db.parse_src_for_compilation(TOP);
        let dependencies = db.parsed_compilation_dependencies(TOP);

        assert_eq!(dependencies.as_ref(), &[TOP, INCLUDED]);
    }

    #[test]
    fn dynamic_include_does_not_load_the_profile_as_buffers() {
        let db = db_with_macro_included_root();
        let closure = db.static_include_closure(TOP);
        assert!(!closure.is_complete(), "{closure:?}");
        assert!(closure.files().is_empty(), "{closure:?}");

        let options = syntax_tree_options_for_file(&db, TOP);
        assert!(
            options.include_buffers.is_empty(),
            "dynamic include must not register every profile file: {:?}",
            options.include_buffers
        );
    }

    #[test]
    fn compilation_plan_records_dynamic_includes_for_authoritative_resolution() {
        let db = db_with_macro_included_root();
        let plan = db.compilation_plan_for_profile(None);

        assert!(plan.dynamic_include_files.contains(&TOP));
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
        db.set_files_with_durability(files, Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::LOW,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        for (file_id, _path, kind, text) in [
            (TOP, top_path, SourceFileKind::SystemVerilog, "module top; endmodule\n"),
            (MANIFEST, manifest_path, SourceFileKind::ProjectManifest, "defines = [\"M=1\"]\n"),
        ] {
            db.set_source_root_id_with_durability(file_id, ROOT, Durability::LOW);
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
        let trace = Trace {
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
            Err(SourcePreprocQueryError::SourceUnavailable(..))
        ));
    }

    #[test]
    fn source_preproc_mapping_records_predefines_by_verified_source_text() {
        let db = db_with_root_file();
        let first_text = materialized_predefine_text("FIRST=1");
        let second_text = materialized_predefine_text("SECOND");
        let trace = Trace {
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

        let Some(PreprocSourceMapping::VirtualFile { file_id: None, path, origin }) =
            source_map.get(first)
        else {
            panic!("first predefine should map to display-only virtual source");
        };
        assert_eq!(path, &expected_path);
        assert_eq!(origin, &PreprocVirtualOrigin::Predefines { profile: None });

        assert_eq!(
            source_map.get(second),
            Some(&PreprocSourceMapping::VirtualFile {
                file_id: None,
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
            Err(SourcePreprocQueryError::DisplayOnlyVirtualSource { .. })
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
        let trace = Trace {
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

        assert!(matches!(
            source_map.get(first),
            Some(PreprocSourceMapping::VirtualFile { file_id: None, .. })
        ));
        assert!(matches!(
            source_map.get(second),
            Some(PreprocSourceMapping::VirtualFile { file_id: None, .. })
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
        let trace = Trace {
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
            Err(SourcePreprocQueryError::SourceUnavailable(..))
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
        let trace = Trace {
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
        let trace = Trace {
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
        let Some(PreprocSourceMapping::VirtualFile { file_id: None, path, origin }) =
            source_map.get(source)
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
            Err(SourcePreprocQueryError::RangeOutOfBounds { .. })
        ));
    }

    #[test]
    fn preproc_virtual_paths_use_reserved_namespace() {
        assert_eq!(
            preproc_virtual_predefines_path(None),
            VfsPath::new_virtual_path("/__vide/preproc/default/predefines.sv".to_owned())
        );
    }
}
