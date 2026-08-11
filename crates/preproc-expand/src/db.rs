use base_db::{
    analysis_snapshot::CompilationContext,
    diagnostics_config::{DiagnosticSource, DiagnosticsConfig},
    project::CompilationProfileId,
    source_db::{SourceDb, SourceFileKind, SourceRootDb},
    source_root::SourceRootId,
};
use rustc_hash::FxHashMap;
use syntax::{
    Compilation, ParserExpectedSyntax, SyntaxDiagnostic, SyntaxTree, SyntaxTreeBuffer,
    SyntaxTreeBufferIds, preproc::Trace,
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

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct PreprocSourceRootQueryKey {
    #[returns(copy)]
    pub source_root_id: SourceRootId,
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub(crate) struct MacroFileQueryKey {
    #[returns(copy)]
    pub macro_file: MacroFileId,
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
    pub preprocessor_trace: Option<Trace>,
}

pub type ParsedProfileUnits = Arc<[(FileId, ParsedCompilationUnit, SyntaxTreeBufferIds)]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProfile {
    pub units: ParsedProfileUnits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationProfileDiagnostics {
    pub diagnostics: Arc<[CompilationDiagnostic]>,
}

fn source_file_identity(db: &dyn SourceDb, file_id: FileId) -> SourceFileIdentity {
    let path = compilation_plan::source_buffer_path(db, file_id).to_string();
    let name = path.clone();
    SourceFileIdentity { name, path }
}

pub(crate) fn path_file_ids(db: &dyn SourceRootDb) -> PathIdentityIndex<FileId> {
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

pub(crate) fn syntax_tree_options_for_file(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    let _span = tracing::info_span!("slang.syntax_tree_options.file", ?file_id).entered();
    let profile_id = db.file_compilation_profile(file_id);
    let context = db.compilation_context_for_file(file_id);
    let identity = source_file_identity(db, file_id);
    let include_buffers = db
        .include_buffers_for_profile(profile_id)
        .iter()
        .filter(|buffer| buffer.path != identity.path)
        .cloned()
        .collect();
    syntax::SyntaxTreeOptions {
        predefines: context.predefines.to_vec(),
        include_paths: context.include_dirs.iter().map(ToString::to_string).collect(),
        include_buffers,
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn syntax_tree_options_for_profile(context: &CompilationContext) -> syntax::SyntaxTreeOptions {
    syntax::SyntaxTreeOptions {
        predefines: context.predefines.to_vec(),
        include_paths: context.include_dirs.iter().map(ToString::to_string).collect(),
        include_buffers: Vec::new(),
        ..syntax::SyntaxTreeOptions::default()
    }
}

fn syntax_tree_options_for_library_map() -> syntax::SyntaxTreeOptions {
    syntax::SyntaxTreeOptions::default()
}

fn syntax_tree_options_for_parser_cursor(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> syntax::SyntaxTreeOptions {
    let profile_id = db.file_compilation_profile(file_id);
    let context = db.compilation_context_for_file(file_id);
    let identity = source_file_identity(db, file_id);
    let include_buffers = if db.file_kind(file_id).is_semantic_compilation_unit() {
        let plan = db.compilation_plan_for_profile(profile_id);
        compilation_plan::compilation_source_buffers_for_plan(db, &plan)
    } else {
        db.include_buffers_for_profile(profile_id).as_ref().clone()
    };
    syntax::SyntaxTreeOptions {
        predefines: context.predefines.to_vec(),
        include_paths: context.include_dirs.iter().map(ToString::to_string).collect(),
        include_buffers: include_buffers
            .into_iter()
            .filter(|buffer| buffer.path != identity.path)
            .collect(),
        ..syntax::SyntaxTreeOptions::default()
    }
}

#[salsa::tracked(returns(clone))]
fn parsed_compilation_unit(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> ParsedCompilationUnit {
    let file_id = key.file_id(db);
    let profile_id = db.file_compilation_profile(file_id);
    let plan = db.compilation_plan_for_profile(profile_id);
    if plan.roots.contains(&file_id) {
        let parsed_profile = db.parsed_profile(profile_id);
        let Some((_, parsed, _)) =
            parsed_profile.units.iter().find(|(root_file_id, _, _)| *root_file_id == file_id)
        else {
            panic!(
                "compilation root {file_id:?} is missing from authoritative parse for profile {profile_id:?}"
            );
        };
        tracing::debug!(
            ?profile_id,
            ?file_id,
            root_count = plan.roots.len(),
            parse_mode = "authoritative",
            "reusing profile root syntax tree"
        );
        return parsed.clone();
    }

    let _span = tracing::info_span!(
        "slang.parse_for_compilation",
        ?profile_id,
        ?file_id,
        parse_mode = "authoritative"
    )
    .entered();
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
            let parsed = SyntaxTree::from_file_in_memory_with_options_and_trace(
                &text,
                &identity.name,
                &identity.path,
                &options,
            );
            ParsedCompilationUnit {
                syntax_tree: parsed.tree,
                preprocessor_trace: Some(parsed.preprocessor_trace),
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

#[salsa::tracked(lru = 128, returns(clone))]
fn parsed_profile(db: &dyn PreprocDb, key: PreprocProfileQueryKey) -> Arc<ParsedProfile> {
    let profile_id = key.profile_id(db);
    let context = db.compilation_context(profile_id);
    let plan = db.compilation_plan_for_profile(profile_id);
    let source_buffers = compilation_plan::compilation_source_buffers_for_plan(db, &plan);
    let root_count = plan.roots.len();
    let _span = tracing::info_span!(
        "slang.profile_parse",
        ?profile_id,
        root_count,
        parse_mode = "authoritative"
    )
    .entered();

    let mut session = Compilation::new_with_top_modules(&context.top_modules);
    session.register_source_buffers(&source_buffers);
    let mut units = Vec::with_capacity(root_count);
    for file_id in plan.roots.iter().copied() {
        let identity = source_file_identity(db, file_id);
        let (syntax_tree, preprocessor_trace) = match db.file_kind(file_id) {
            SourceFileKind::SystemVerilog => {
                let options = syntax_tree_options_for_profile(&context);
                let syntax_tree =
                    session.parse_syntax_tree_from_buffer(&identity.name, &identity.path, &options);
                let preprocessor_trace = Some(syntax_tree.preprocessor_trace());
                (syntax_tree, preprocessor_trace)
            }
            SourceFileKind::LibraryMap => {
                let options = syntax_tree_options_for_library_map();
                (
                    session.parse_library_map_syntax_tree_from_buffer(
                        &identity.name,
                        &identity.path,
                        &options,
                    ),
                    None,
                )
            }
            SourceFileKind::IncludeHeader | SourceFileKind::ProjectManifest => {
                panic!("non-compilation unit {file_id:?} appeared in profile roots")
            }
        };
        let buffer_ids = syntax_tree.buffer_ids();
        tracing::debug!(
            ?profile_id,
            ?file_id,
            root_count,
            parse_mode = "authoritative",
            "profile root syntax tree parsed"
        );
        units.push((
            file_id,
            ParsedCompilationUnit { syntax_tree, preprocessor_trace },
            buffer_ids,
        ));
    }

    tracing::debug!(
        ?profile_id,
        root_count = units.len(),
        parse_mode = "authoritative",
        "profile authoritative parse complete"
    );
    Arc::new(ParsedProfile { units: Arc::from(units) })
}

#[salsa::tracked(lru = 128, returns(clone))]
fn parse_src_for_compilation(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> SyntaxTree {
    db.parsed_compilation_unit(key.file_id(db)).syntax_tree.clone()
}

pub fn set_parse_lru_capacity(db: &mut dyn PreprocDb, capacity: usize) {
    parsed_profile::set_lru_capacity(db, capacity);
    parse_src_for_compilation::set_lru_capacity(db, capacity);
}

/// Parser expectations at one cursor offset.
///
/// Expectations are completion-only metadata. Keep them out of authoritative
/// compilation trees and ask Slang for a cursor-scoped parse when completion
/// needs them. The fallback receives the same profile buffers and options as
/// the authoritative parse, so an unsaved include cannot silently come from
/// disk.
#[salsa::tracked(returns(clone))]
fn parser_expected_syntax(
    db: &dyn PreprocDb,
    file_id: FileId,
    offset: TextSize,
) -> Arc<[ParserExpectedSyntax]> {
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

#[salsa::tracked(returns(clone))]
fn parse_diagnostics(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Arc<[SyntaxDiagnostic]> {
    let file_id = key.file_id(db);
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
    fn file_query_key(&self, file_id: FileId) -> PreprocFileQueryKey {
        PreprocFileQueryKey::new(self, file_id)
    }

    fn profile_query_key(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> PreprocProfileQueryKey {
        PreprocProfileQueryKey::new(self, profile_id)
    }

    pub fn compilation_plan_for_root(&self, source_root_id: SourceRootId) -> Arc<CompilationPlan> {
        compilation_plan_for_root(self, PreprocSourceRootQueryKey::new(self, source_root_id))
    }

    pub fn compilation_plan_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationPlan> {
        compilation_plan_for_profile(self, self.profile_query_key(profile_id))
    }

    pub fn compilation_context(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<CompilationContext> {
        compilation_context(self, self.profile_query_key(profile_id))
    }

    pub fn compilation_context_for_file(&self, file_id: FileId) -> Arc<CompilationContext> {
        compilation_context_for_file(self, self.file_query_key(file_id))
    }

    pub fn compilation_profile_diagnostics(
        &self,
        profile_id: CompilationProfileId,
    ) -> Arc<CompilationProfileDiagnostics> {
        compilation_profile_diagnostics(self, self.profile_query_key(Some(profile_id)))
    }

    pub fn include_buffers_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<Vec<SyntaxTreeBuffer>> {
        include_buffers_for_profile(self, self.profile_query_key(profile_id))
    }

    pub fn source_preproc_model(
        &self,
        file_id: FileId,
    ) -> Arc<Result<MappedSourcePreprocModel, SourcePreprocQueryError>> {
        source_preproc_model(self, self.file_query_key(file_id))
    }

    pub fn source_preproc_context_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<SourcePreprocContextIndex> {
        source_preproc_context_index_for_profile(self, self.profile_query_key(profile_id))
    }

    pub fn source_preproc_contexts_for_file(
        &self,
        file_id: FileId,
    ) -> Arc<SourcePreprocRelevantContexts> {
        source_preproc_contexts_for_file(self, self.file_query_key(file_id))
    }

    pub fn parsed_compilation_unit(&self, file_id: FileId) -> ParsedCompilationUnit {
        parsed_compilation_unit(self, self.file_query_key(file_id))
    }

    pub fn parsed_profile(&self, profile_id: Option<CompilationProfileId>) -> Arc<ParsedProfile> {
        parsed_profile(self, self.profile_query_key(profile_id))
    }

    pub fn parse_src_for_compilation(&self, file_id: FileId) -> SyntaxTree {
        parse_src_for_compilation(self, self.file_query_key(file_id))
    }

    pub fn parser_expected_syntax(
        &self,
        file_id: FileId,
        offset: TextSize,
    ) -> Arc<[ParserExpectedSyntax]> {
        parser_expected_syntax(self, file_id, offset)
    }

    pub fn parse_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
        parse_diagnostics(self, self.file_query_key(file_id))
    }

    pub fn file_compilation_diagnostics(&self, file_id: FileId) -> Arc<[CompilationDiagnostic]> {
        file_compilation_diagnostics(self, self.file_query_key(file_id))
    }

    pub fn semantic_diagnostics(&self, file_id: FileId) -> Arc<[SyntaxDiagnostic]> {
        semantic_diagnostics(self, self.file_query_key(file_id))
    }

    pub fn source_root_semantic_diagnostics(
        &self,
        file_id: FileId,
    ) -> Arc<[(FileId, SyntaxDiagnostic)]> {
        source_root_semantic_diagnostics(self, self.file_query_key(file_id))
    }

    pub fn macro_expansion(&self, macro_file: MacroFileId) -> Arc<ExpandResult<ExpansionInfo>> {
        macro_file::macro_expansion_query(self, MacroFileQueryKey::new(self, macro_file))
    }

    pub fn parse(&self, file_id: HirFileId) -> SyntaxTree {
        parse(self, file_id)
    }

    pub fn trace_index(&self, model_file: FileId) -> Arc<TraceIndex> {
        macro_file::trace_index_query(self, self.file_query_key(model_file))
    }

    pub fn file_macro_coverage(&self, file_id: FileId) -> Arc<MacroCoverage> {
        file_macro_coverage_query(self, file_id)
    }

    pub fn macro_reference_index_for_profile(
        &self,
        profile_id: Option<CompilationProfileId>,
    ) -> Arc<MacroReferenceIndex> {
        macro_reference_index_for_profile_query(self, self.profile_query_key(profile_id))
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

#[salsa::tracked(returns(clone))]
fn compilation_plan_for_root(
    db: &dyn PreprocDb,
    key: PreprocSourceRootQueryKey,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_source_root(db, key.source_root_id(db)))
}

#[salsa::tracked(returns(clone))]
fn compilation_plan_for_profile(
    db: &dyn PreprocDb,
    key: PreprocProfileQueryKey,
) -> Arc<CompilationPlan> {
    Arc::new(CompilationPlan::for_profile(db, key.profile_id(db)))
}

#[salsa::tracked(returns(clone))]
fn compilation_context(db: &dyn PreprocDb, key: PreprocProfileQueryKey) -> Arc<CompilationContext> {
    let profile_id = key.profile_id(db);
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

#[salsa::tracked(returns(clone))]
fn compilation_context_for_file(
    db: &dyn PreprocDb,
    key: PreprocFileQueryKey,
) -> Arc<CompilationContext> {
    let profile_id = db.file_compilation_profile(key.file_id(db));
    db.compilation_context(profile_id)
}

#[salsa::tracked(returns(clone))]
fn compilation_profile_diagnostics(
    db: &dyn PreprocDb,
    key: PreprocProfileQueryKey,
) -> Arc<CompilationProfileDiagnostics> {
    let profile_id =
        key.profile_id(db).expect("compilation diagnostics require a concrete profile");
    let config = db.diagnostics_config();
    let _span =
        tracing::info_span!("slang.profile_compilation", ?profile_id, parse_mode = "authoritative")
            .entered();
    if !config.enabled {
        return Arc::new(CompilationProfileDiagnostics { diagnostics: Arc::from(Vec::new()) });
    }

    let context = db.compilation_context(Some(profile_id));
    let parsed_profile = db.parsed_profile(Some(profile_id));
    let mut compilation = Compilation::new_with_top_modules(&context.top_modules);
    let mut buffer_file_ids = FxHashMap::default();
    let path_file_ids = path_file_ids(db);

    for (file_id, parsed_unit, buffer_ids) in parsed_profile.units.iter() {
        compilation.add_syntax_tree(&parsed_unit.syntax_tree);
        let buffer_ids_for_map = buffer_ids.clone();
        insert_buffer_file_ids(&mut buffer_file_ids, &path_file_ids, buffer_ids_for_map, *file_id);
    }

    let diagnostics =
        compilation_diagnostics_from_compilation(&config, &compilation, &buffer_file_ids);
    Arc::new(CompilationProfileDiagnostics { diagnostics })
}

fn compilation_diagnostics_from_compilation(
    config: &DiagnosticsConfig,
    compilation: &Compilation,
    buffer_file_ids: &FxHashMap<u32, FileId>,
) -> Arc<[CompilationDiagnostic]> {
    if !config.enabled || (!config.parse.enabled && !config.semantic.enabled) {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    }

    let mut diagnostics = Vec::new();
    if config.parse.enabled {
        let raw_diagnostics = {
            let _span = tracing::info_span!("slang.semantic.parse_diagnostics").entered();
            compilation.parse_diagnostics_with_options(&slang_warning_options(config))
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

    if config.semantic.enabled {
        let raw_semantic_diagnostics = {
            let _span = tracing::info_span!("slang.semantic.raw_diagnostics").entered();
            compilation.semantic_diagnostics_with_options(&slang_warning_options(config))
        };
        let raw_semantic_diagnostic_count = raw_semantic_diagnostics.len();
        let mut unmapped_semantic_buffer_count = 0usize;
        let mut ignored_semantic_diagnostic_count = 0usize;
        {
            let _span = tracing::info_span!(
                "slang.semantic.map_diagnostics",
                raw_semantic_diagnostic_count
            )
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
    }

    Arc::from(diagnostics)
}

#[salsa::tracked(returns(clone))]
fn include_buffers_for_profile(
    db: &dyn PreprocDb,
    key: PreprocProfileQueryKey,
) -> Arc<Vec<SyntaxTreeBuffer>> {
    let profile_id = key.profile_id(db);
    let plan = db.compilation_plan_for_profile(profile_id);
    Arc::new(compilation_plan::include_buffers_for_plan(db, &plan))
}

#[salsa::tracked(returns(clone))]
fn semantic_diagnostics(db: &dyn PreprocDb, key: PreprocFileQueryKey) -> Arc<[SyntaxDiagnostic]> {
    let file_id = key.file_id(db);
    Arc::from(
        db.source_root_semantic_diagnostics(file_id)
            .iter()
            .filter_map(|(diag_file_id, diag)| (*diag_file_id == file_id).then_some(diag.clone()))
            .collect::<Vec<_>>(),
    )
}

#[salsa::tracked(returns(clone))]
fn file_compilation_diagnostics(
    db: &dyn PreprocDb,
    key: PreprocFileQueryKey,
) -> Arc<[CompilationDiagnostic]> {
    let file_id = key.file_id(db);
    let source_root_id = db.source_root_id(file_id);
    let config = db.diagnostics_config();
    if !config.enabled || db.file_is_project_ignored(file_id) {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    }

    let project_config = db.project_config();
    let Some(profile_id) = project_config.profile_for_root(source_root_id) else {
        return Arc::from(Vec::<CompilationDiagnostic>::new());
    };
    db.compilation_profile_diagnostics(profile_id).diagnostics.clone()
}

#[salsa::tracked(returns(clone))]
fn source_root_semantic_diagnostics(
    db: &dyn PreprocDb,
    key: PreprocFileQueryKey,
) -> Arc<[(FileId, SyntaxDiagnostic)]> {
    let file_id = key.file_id(db);
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
    use syntax::{SourceBufferId, SourceBufferOrigin, SyntaxTreeOptions, preproc::Trace};
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
        db.set_files_with_durability(files, Durability::HIGH);
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
    fn parsed_profile_uses_the_compilation_context() {
        let mut db = db_with_root_file();
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::LOW);
        let profile = db.parsed_profile(None);
        assert_eq!(profile.units.len(), 1);
        let tree = profile.units[0].1.syntax_tree.clone();
        let root = tree.root();
        assert!(root.children().next().is_some());
    }

    #[test]
    fn parser_expectations_are_cursor_scoped_outside_authoritative_tree() {
        let mut db = db_with_root_file();
        let text = "module top; always begin begin end endmodule\n";
        db.set_file_text_with_durability(TOP, Arc::from(text), Durability::LOW);
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::LOW);

        let tree = db.parsed_profile(None).units[0].1.syntax_tree.clone();
        assert!(tree.expected_syntax_at(28).is_empty());
        assert!(!db.parser_expected_syntax(TOP, TextSize::from(28)).is_empty());
    }

    #[test]
    fn root_scoped_compilation_units_reuse_the_authoritative_parse() {
        let mut db = db_with_root_file();
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::LOW);

        let profile_tree = db.parsed_profile(None).units[0].1.syntax_tree.clone();
        let compilation_tree = db.parsed_compilation_unit(TOP).syntax_tree;

        assert_eq!(profile_tree, compilation_tree);
    }

    #[test]
    fn profile_compilation_units_reuse_the_authoritative_profile_parse() {
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

        let profile_tree =
            db.parsed_profile(Some(CompilationProfileId(0))).units[0].1.syntax_tree.clone();
        let compilation_tree = db.parsed_compilation_unit(TOP).syntax_tree;

        assert_eq!(profile_tree, compilation_tree);
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
