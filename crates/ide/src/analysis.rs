use std::{
    ops::{Deref, Range},
    panic::AssertUnwindSafe,
    sync::Arc as StdArc,
};

use base_db::{
    Cancelled,
    analysis_snapshot::AnalysisSnapshotId,
    project::CompilationProfileId,
    source_db::{SourceDb, SourceRootDb},
    source_root::{SourceRootId, SourceRootRole},
};
use design_graph::DesignGraphDb;
use hir_def::{def_id::DefId, pathres::ResolutionContext};
use preproc_expand::{compilation_plan::CompilationPlan, db::PreprocDb};
use triomphe::Arc;
use utils::{
    cancellation::CancellationToken,
    line_index::{LineIndex, TextRange},
    lines::LineInfo,
    text_edit::TextEdit,
};
use vfs::FileId;

use crate::{
    Cancellable, FilePosition, RangeInfo,
    code_action::{self, CodeAction, CodeActionResolveStrategy},
    code_lens::{self, CodeLens, CodeLensConfig, CodeLensKind},
    compile::Compiler,
    completion::{CompletionItem, context::TriggerChar},
    db::{line_index_db::LineIndexDb, root_db::RootDb},
    diagnostics,
    document_highlight::{self, DocumentHighlight, DocumentHighlightConfig},
    document_symbols::{self, DocumentSymbol},
    folding_ranges::{self, Fold},
    formatting::{self, FmtConfig},
    goto_declaration, goto_definition, hover,
    incrementality::ProductStore,
    inlay_hint::{self, InlayHint, InlayHintConfig},
    markup::Markup,
    navigation_target::NavTarget,
    reference_support::{self, ModuleCallEdge},
    references::{self, References, ReferencesConfig},
    rename::{self, RenameConfig, RenameResult},
    selection_ranges,
    semantic_tokens::{self, SemaToken, SemaTokenConfig},
    signature_help::{self, SignatureHelp, SignatureHelpConfig},
    source_change::SourceChange,
    workspace_symbols::{self, WorkspaceSymbol},
};

/// One IDE read: salsa [`RootDb`] plus a way to [`Compiler::compile`] a radius.
///
/// [`ProductStore`] is parse-deps for paid files, forked so older snapshots
/// keep the previous paid-file set. `snapshot_id` / `salsa_revision` are
/// cancellation and staleness checks, not peer clocks of the compiler.
pub struct AnalysisSnapshot {
    pub(crate) db: RootDb,
    pub(crate) store: Arc<ProductStore>,
    pub(crate) snapshot_id: AnalysisSnapshotId,
    pub(crate) salsa_revision: base_db::salsa::Revision,
    pub(crate) compiler: StdArc<parking_lot::Mutex<Compiler>>,
}

/// Read view of one IDE request: the database and the keystroke [`Compiler`].
///
/// [`Self::parse_file`] records the file as paid so later this-file lexical
/// resolution can look at that file's `HirFileId::Macro` owner table. It does
/// not merge generated names into the catalog.
///
/// Types, `::`, `.` members, and keystroke goto wait on
/// [`Compiler::compile`] of the file closure. Hierarchy and specialized
/// instances compile the profile on the calling thread.
pub(crate) struct AnalysisContext<'a> {
    pub(crate) db: &'a RootDb,
    pub(crate) store: &'a ProductStore,
    pub(crate) compiler: Option<&'a parking_lot::Mutex<Compiler>>,
}

impl Deref for AnalysisContext<'_> {
    type Target = RootDb;

    fn deref(&self) -> &RootDb {
        self.db
    }
}

impl AnalysisContext<'_> {
    pub(crate) fn new<'a>(
        db: &'a RootDb,
        store: &'a ProductStore,
        compiler: Option<&'a parking_lot::Mutex<Compiler>>,
    ) -> AnalysisContext<'a> {
        AnalysisContext { db, store, compiler }
    }

    pub(crate) fn compile_files(
        &self,
        files: impl IntoIterator<Item = vfs::FileId>,
        options: &crate::compile::CompileOptions,
    ) -> crate::compile::CompilationArtifact {
        let compiler = self.compiler.expect("compile needs the session compiler");
        compiler.lock().compile(self.db, files, options)
    }

    /// File-closure compile. Must not wait on a profile compilation.
    pub(crate) fn keystroke_compilation(
        &self,
        file_id: vfs::FileId,
    ) -> crate::compile::CompilationArtifact {
        let closure = crate::compile::file_closure(self.db, file_id);
        let options = crate::compile::CompileOptions::for_file(self.db, file_id);
        self.compile_files(closure.files().iter().copied(), &options)
    }

    pub(crate) fn profile_compilation(
        &self,
        profile: Option<CompilationProfileId>,
    ) -> crate::compile::CompilationArtifact {
        let plan = <dyn PreprocDb>::compilation_plan_for_profile(self.db, profile);
        let options = crate::compile::CompileOptions::for_profile(self.db, profile);
        crate::compile::Compiler::compile_isolated(
            self.db,
            plan.all_file_ids().iter().copied(),
            &options,
        )
    }

    pub(crate) fn profile_diagnostics(
        &self,
        profile: CompilationProfileId,
    ) -> Vec<preproc_expand::db::CompilationDiagnostic> {
        let plan = <dyn PreprocDb>::compilation_plan_for_profile(self.db, Some(profile));
        let options = crate::compile::CompileOptions::for_profile(self.db, Some(profile));
        crate::compile::Compiler::diagnostics_isolated(
            self.db,
            plan.all_file_ids().iter().copied(),
            &options,
            self.db.diagnostics_config().as_ref(),
        )
    }

    pub(crate) fn semantics(&self) -> hir_semantics::semantics::Semantics<'_, RootDb> {
        hir_semantics::semantics::Semantics::new_with_context(self.db, self.resolution())
    }

    /// Parse one file without building `$unit` or the design map.
    pub(crate) fn parse_file(&self, file_id: FileId) -> syntax::SyntaxTree {
        let (tree, dependencies) = self.db.parse_src_with_dependencies(file_id);
        self.store.record_parse_dependencies(file_id, dependencies);
        tree
    }

    pub(crate) fn source_semantic_map(
        &self,
        file_id: FileId,
    ) -> Arc<preproc_expand::macro_file::SourceSemanticMap> {
        let db: &dyn preproc_expand::db::PreprocDb = self.db;
        db.source_semantic_map(file_id)
    }

    pub(crate) fn file_facts(&self, file_id: FileId) -> Arc<design_graph::FileFacts> {
        self.db.file_facts(file_id)
    }

    pub(crate) fn unit_catalog(&self) -> triomphe::Arc<design_graph::UnitCatalog> {
        <dyn DesignGraphDb>::source_unit_catalog(self.db)
    }

    /// This-file lexical nameres and package imports. Not types / `::` /
    /// members / other-file hierarchy — those compile the file closure.
    pub(crate) fn resolution(&self) -> Arc<ResolutionContext> {
        ResolutionContext::from_locator(
            self.db,
            self.unit_catalog(),
            Arc::from(self.store.paid_files()),
        )
    }

    pub(crate) fn recursive_rename_closure(
        &self,
        def: DefId,
        visibility: crate::ScopeVisibility,
        single_file: Option<FileId>,
    ) -> Arc<Vec<DefId>> {
        Arc::new(crate::rename::recursive_rename_closure_impl(self, def, visibility, single_file))
    }
}

impl AnalysisSnapshot {
    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub fn project_anchor(
        &self,
        anchor: crate::anchor::Anchor,
    ) -> Cancellable<Option<crate::anchor::ProjectedAnchor>> {
        self.with_db(|ctx| crate::anchor::project(ctx, &anchor))
    }

    pub fn ast_id_at_range(
        &self,
        file_id: FileId,
        range: utils::line_index::TextRange,
    ) -> Cancellable<Option<hir_def::ast_id_map::SourceAstId>> {
        self.with_db(|ctx| crate::anchor::ast_id_at_range(ctx.db, file_id, range))
    }

    fn with_db<F, T>(&self, f: F) -> Cancellable<T>
    where
        F: FnOnce(&AnalysisContext<'_>) -> T + std::panic::UnwindSafe,
    {
        debug_assert_eq!(
            base_db::salsa::plumbing::current_revision(&self.db),
            self.salsa_revision,
            "an AnalysisSnapshot must never cross Salsa revisions",
        );
        let _span = tracing::debug_span!("ide.analysis", snapshot_id = ?self.snapshot_id).entered();
        let ctx = AnalysisContext::new(&self.db, &self.store, Some(&self.compiler));
        Cancelled::catch(AssertUnwindSafe(|| f(&ctx)))
    }

    pub fn line_index(&self, file_id: FileId) -> Cancellable<Arc<LineIndex>> {
        self.with_db(|db| db.line_index(file_id))
    }

    pub fn file_text(&self, file_id: FileId) -> Cancellable<Arc<str>> {
        self.with_db(|db| db.file_text(file_id))
    }

    pub fn file_ids(&self) -> Cancellable<Vec<FileId>> {
        self.with_db(|db| db.files().iter().copied().collect())
    }

    pub fn diagnostics(&self, file_id: FileId) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::analysis_diagnostics(db, file_id))
    }

    pub fn source_root_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::source_root_diagnostics(db, file_id))
    }

    pub fn compilation_profile_slang_diagnostics(
        &self,
        profile_id: CompilationProfileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|ctx| {
            diagnostics::materialize_compiler_diagnostics(ctx.profile_diagnostics(profile_id))
        })
    }

    pub fn file_vide_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::vide_diagnostics(db.db, db.resolution().as_ref(), file_id))
    }

    pub fn parse_diagnostics(&self, file_id: FileId) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::parse_diagnostics_on(db, file_id))
    }

    pub fn source_root_file_ids(&self, file_id: FileId) -> Cancellable<Vec<FileId>> {
        self.with_db(|db| diagnostics::source_root_file_ids(db, file_id))
    }

    pub fn source_root_role(&self, file_id: FileId) -> Cancellable<SourceRootRole> {
        self.with_db(|db| diagnostics::source_root_role(db, file_id))
    }

    pub fn source_root_id(&self, file_id: FileId) -> Cancellable<SourceRootId> {
        self.with_db(|db| db.source_root_id(file_id))
    }

    pub fn file_compilation_profile(
        &self,
        file_id: FileId,
    ) -> Cancellable<Option<CompilationProfileId>> {
        self.with_db(|db| db.file_compilation_profile(file_id))
    }

    pub fn has_compilation_profiles(&self) -> Cancellable<bool> {
        self.with_db(|db| db.project_config().has_compilation_profiles())
    }

    pub fn compilation_profile_ids(&self) -> Cancellable<Vec<CompilationProfileId>> {
        self.with_db(|db| db.project_config().profile_ids())
    }

    pub fn compilation_profile_file_ids(
        &self,
        profile_id: CompilationProfileId,
    ) -> Cancellable<Vec<FileId>> {
        self.with_db(|db| db.compilation_plan_for_profile(Some(profile_id)).all_file_ids())
    }

    pub fn compilation_profile_instances(
        &self,
        profile_id: CompilationProfileId,
    ) -> Cancellable<Vec<(crate::hier::HierPath, FileId, utils::line_index::TextRange)>> {
        self.with_db(|ctx| {
            let mut artifact = ctx.profile_compilation(Some(profile_id));
            let crate::compile::QueryStatus::Ready(Some(rows)) = artifact.query_instances() else {
                return Vec::new();
            };
            rows.into_iter()
                .filter_map(|row| {
                    let file = artifact.file_id_for_path(&row.file)?;
                    let tail = row.path.rsplit('.').next().unwrap_or(row.path.as_str());
                    let name_len = tail.find('[').unwrap_or(tail.len());
                    let start = utils::line_index::TextSize::from(row.offset as u32);
                    let range = utils::line_index::TextRange::new(
                        start,
                        start + utils::line_index::TextSize::from(name_len as u32),
                    );
                    Some((crate::hier::HierPath::new(row.path), file, range))
                })
                .collect()
        })
    }

    pub fn compilation_plan(&self, file_id: FileId) -> Cancellable<Arc<CompilationPlan>> {
        self.with_db(|db| db.db.compilation_plan_for_root(db.source_root_id(file_id)))
    }
}

impl AnalysisSnapshot {
    pub fn goto_definition(
        &self,
        position: FilePosition,
    ) -> Cancellable<Option<RangeInfo<Vec<NavTarget>>>> {
        self.with_db(|db| goto_definition::goto_definition(db, position))
    }

    pub fn goto_declaration(
        &self,
        position: FilePosition,
    ) -> Cancellable<Option<RangeInfo<Vec<NavTarget>>>> {
        self.with_db(|db| goto_declaration::goto_declaration(db, position))
    }

    pub fn document_symbol(&self, file_id: FileId) -> Cancellable<Vec<DocumentSymbol>> {
        self.with_db(|db| document_symbols::document_symbols(db.db, file_id))
    }

    pub fn workspace_symbol(
        &self,
        query: &str,
        file_ids: Vec<FileId>,
    ) -> Cancellable<Vec<WorkspaceSymbol>> {
        self.with_db(|db| workspace_symbols::workspace_symbols(db, query, file_ids))
    }

    pub fn document_highlight(
        &self,
        position: FilePosition,
        config: DocumentHighlightConfig,
    ) -> Cancellable<Option<Vec<DocumentHighlight>>> {
        self.with_db(|db| document_highlight::document_highlight(db, position, config))
    }

    pub fn references(
        &self,
        position: FilePosition,
        config: ReferencesConfig,
    ) -> Cancellable<Option<Vec<References>>> {
        self.with_db(|db| references::references(db, position, config))
    }

    pub fn module_incoming_calls(
        &self,
        file_id: FileId,
        name_range: TextRange,
    ) -> Cancellable<Vec<ModuleCallEdge>> {
        self.with_db(|db| reference_support::incoming_module_edges(db, file_id, name_range))
    }

    pub fn module_outgoing_calls(
        &self,
        file_id: FileId,
        name_range: TextRange,
    ) -> Cancellable<Vec<ModuleCallEdge>> {
        self.with_db(|db| reference_support::outgoing_module_edges(db, file_id, name_range))
    }

    pub fn prepare_rename(
        &self,
        position: FilePosition,
        config: RenameConfig,
    ) -> Cancellable<RenameResult<TextRange>> {
        self.with_db(|db| rename::prepare_rename(db, position, config))
    }

    pub fn rename(
        &self,
        position: FilePosition,
        config: RenameConfig,
        new_name: &str,
    ) -> Cancellable<RenameResult<SourceChange>> {
        self.with_db(|db| rename::rename(db, position, config, new_name))
    }

    pub fn rename_expansion_info(
        &self,
        position: FilePosition,
        config: RenameConfig,
    ) -> Cancellable<RenameResult<rename::RecursiveRenameInfo>> {
        self.with_db(|db| rename::rename_expansion_info(db, position, config))
    }

    pub fn expanded_rename(
        &self,
        position: FilePosition,
        config: RenameConfig,
        new_name: &str,
    ) -> Cancellable<RenameResult<SourceChange>> {
        self.with_db(|db| rename::expanded_rename(db, position, config, new_name))
    }

    pub fn rename_conflict_info(
        &self,
        position: FilePosition,
        config: RenameConfig,
        new_name: &str,
        recursive: bool,
    ) -> Cancellable<RenameResult<rename::RenameCollisionInfo>> {
        self.with_db(|db| rename::rename_conflict_info(db, position, config, new_name, recursive))
    }

    pub fn format(
        &self,
        file_id: FileId,
        line_range: Option<Range<usize>>,
        line_info: &LineInfo,
        config: FmtConfig,
        cancellation: CancellationToken,
    ) -> Cancellable<anyhow::Result<Option<TextEdit>>> {
        self.with_db(|db| {
            formatting::format(db, file_id, line_range, line_info, config, &cancellation)
        })
    }

    pub fn format_on_type(
        &self,
        position: FilePosition,
        ch: String,
        line_info: &LineInfo,
        config: FmtConfig,
        cancellation: CancellationToken,
    ) -> Cancellable<anyhow::Result<Option<TextEdit>>> {
        self.with_db(|db| {
            formatting::format_on_type(db, position, ch, line_info, config, &cancellation)
        })
    }

    pub fn selection_ranges(&self, position: FilePosition) -> Cancellable<Vec<TextRange>> {
        self.with_db(|db| selection_ranges::selection_ranges(db, position))
    }

    pub fn folding_ranges(&self, file_id: FileId) -> Cancellable<Vec<Fold>> {
        self.with_db(|db| folding_ranges::folding_ranges(db, file_id))
    }

    pub fn hover(&self, position: FilePosition) -> Cancellable<Option<RangeInfo<Markup>>> {
        self.with_db(|db| hover::hover(db, position))
    }

    pub fn inlay_hint(
        &self,
        file_id: FileId,
        range: TextRange,
        config: InlayHintConfig,
    ) -> Cancellable<Vec<InlayHint>> {
        self.with_db(|db| {
            inlay_hint::inlay_hint(db, db.resolution().as_ref(), file_id, range, config)
        })
    }

    pub fn code_lens(&self, file_id: FileId, config: CodeLensConfig) -> Cancellable<Vec<CodeLens>> {
        self.with_db(|db| code_lens::code_lens(db, config, file_id))
    }

    pub fn code_lens_resolve(&self, kind: CodeLensKind) -> Cancellable<CodeLensKind> {
        self.with_db(|db| code_lens::code_lens_resolve(db, kind))
    }

    pub fn semantic_tokens(
        &self,
        file_id: FileId,
        config: SemaTokenConfig,
        range: Option<TextRange>,
    ) -> Cancellable<Vec<SemaToken>> {
        self.with_db(|db| semantic_tokens::semantic_tokens(db, config, file_id, range))
    }

    pub fn signature_help(
        &self,
        position: FilePosition,
        config: SignatureHelpConfig,
    ) -> Cancellable<Option<SignatureHelp>> {
        self.with_db(|db| signature_help::signature_help(db, position, config))
    }

    pub fn completions_with_trigger(
        &self,
        position: FilePosition,
        trigger: Option<TriggerChar>,
    ) -> Cancellable<Vec<CompletionItem>> {
        self.with_db(|db| crate::completion::completions(db, position, trigger))
    }

    pub fn code_action(
        &self,
        file_id: FileId,
        range: TextRange,
        diagnostics: &[crate::diagnostics::Diagnostic],
        resolve_strategy: CodeActionResolveStrategy,
    ) -> Cancellable<Vec<CodeAction>> {
        self.with_db(|db| {
            code_action::code_action(db, file_id, range, diagnostics, resolve_strategy)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use hir_def::design_map::PACKAGE_EXPORT_CLOSURE_RUNS;

    /// `semantics()` rebuilds [`super::AnalysisContext::resolution`] each time.
    /// The workspace-level export closure must not re-walk every package on
    /// every one of those calls.
    #[test]
    fn package_export_closure_runs_once_per_request() {
        let (host, _) = crate::test_utils::setup_marked_files(&[
            ("/a.sv", "package a;\n  int x;\nendpackage\n"),
            ("/b.sv", "package b;\n  int y;\nendpackage\n"),
            ("/c.sv", "package c;\n  int z;\nendpackage\n"),
            ("/top.sv", "module top;\n  int w;\nendmodule\n"),
        ]);
        let package_count = host.ctx().unit_catalog().packages().count() as u32;
        assert_eq!(
            package_count, 3,
            "fixture must have three packages so per-package work is visible"
        );

        PACKAGE_EXPORT_CLOSURE_RUNS.with(|runs| runs.set(0));
        let ctx = host.ctx();
        let _ = ctx.resolution();
        let after_first = PACKAGE_EXPORT_CLOSURE_RUNS.with(Cell::get);
        let _ = ctx.semantics();
        let _ = ctx.resolution();
        let closure_runs = PACKAGE_EXPORT_CLOSURE_RUNS.with(Cell::get);
        assert!(
            after_first <= 1,
            "first resolution() may hit a prewarm memo (0) or compute once (1), not {after_first}"
        );
        assert_eq!(
            closure_runs, after_first,
            "later resolution()/semantics() must not re-execute the closure (first={after_first} after={closure_runs})"
        );
    }

    #[test]
    fn to_owner_traffic_on_a_typical_request() {
        use hir_def::unit::TO_OWNER_RUNS;

        let (host, files) = crate::test_utils::setup_marked_files(&[
            ("/a.sv", "package a;\n  int x;\nendpackage\n"),
            ("/b.sv", "package b;\n  import a::*;\n  int y;\nendpackage\n"),
            ("/c.sv", "package c;\n  import b::*;\n  int z;\nendpackage\n"),
            ("/top.sv", "module top;\n  int /*marker:w*/w;\nendmodule\n"),
        ]);
        let file_id = files[3].0;
        let offset = files[3].2["w"];
        TO_OWNER_RUNS.with(|runs| runs.set(0));
        let _ = host.make_analysis().hover(crate::FilePosition { file_id, offset }).unwrap();
        let _ =
            host.make_analysis().goto_definition(crate::FilePosition { file_id, offset }).unwrap();
        let calls = TO_OWNER_RUNS.with(Cell::get);
        println!("t6.to_owner_calls\t{calls}");
        assert_eq!(calls, 0, "T6 removed the UnitId→OwnerId name bridge");
    }

    /// T6 form B: shipped resolution must not project L0 `UnitId` → `OwnerId`
    /// by name. The T5 counter is the production bridge; it must stay at 0.
    #[test]
    fn shipped_request_does_not_project_l0_unit_ids() {
        use hir_def::unit::TO_OWNER_RUNS;

        let (host, files) = crate::test_utils::setup_marked_files(&[
            ("/a.sv", "package a;\n  int x;\nendpackage\n"),
            ("/b.sv", "package b;\n  import a::*;\n  int y;\nendpackage\n"),
            ("/c.sv", "package c;\n  import b::*;\n  int z;\nendpackage\n"),
            ("/top.sv", "module top;\n  int /*marker:w*/w;\nendmodule\n"),
        ]);
        let file_id = files[3].0;
        let offset = files[3].2["w"];
        TO_OWNER_RUNS.with(|runs| runs.set(0));
        let _ = host.make_analysis().hover(crate::FilePosition { file_id, offset }).unwrap();
        let _ =
            host.make_analysis().goto_definition(crate::FilePosition { file_id, offset }).unwrap();
        let calls = TO_OWNER_RUNS.with(Cell::get);
        assert_eq!(
            calls, 0,
            "shipped hover+goto must not project L0 UnitId by name (to_owner={calls})"
        );
    }

    /// P5.1: UnitCatalog is `name → files`. Duplicate CU names are locator
    /// search hits, not a Unique/files[0] binding.
    #[test]
    fn catalog_duplicate_names_are_locator_hits_not_a_unique_binding() {
        let (host, files) = crate::test_utils::setup_marked_files(&[
            ("/a.sv", "module child;\nendmodule\n"),
            ("/b.sv", "module child;\nendmodule\n"),
            ("/top.sv", "module /*marker:top*/top;\n  child u();\nendmodule\n"),
        ]);
        let a = files[0].0;
        let b = files[1].0;
        let top = files[2].0;
        let top_start = files[2].2["top"];
        let top_range = utils::line_index::TextRange::new(
            top_start,
            top_start + utils::line_index::TextSize::of("top"),
        );

        let located = host.ctx().unit_catalog().files_named("child");
        assert_eq!(located.len(), 2, "locator must return both files, not files[0]: {located:?}");
        assert!(located.contains(&a) && located.contains(&b), "{located:?}");

        let outgoing = crate::reference_support::outgoing_module_edges(&host.ctx(), top, top_range);
        let mut callee_files: Vec<_> = outgoing.iter().map(|edge| edge.callee.file_id).collect();
        callee_files.sort_by_key(|file| file.index());
        assert_eq!(
            callee_files,
            vec![a, b],
            "call edges must list both locator hits, not Unique/files[0] or empty: {outgoing:?}"
        );
    }

    /// P5.1: production must not ask the catalog for Unique UnitId binding.
    #[test]
    fn production_catalog_lookups_are_file_locators() {
        let packages = ["packages", "named"].join("_");
        let modules = ["modules", "named"].join("_");
        let type_units = ["type_units", "named"].join("_");
        let candidates = ["graph", "candidates"].join(".");
        for (path, src) in [
            ("design_unit.rs", include_str!("design_unit.rs")),
            ("reference_support.rs", include_str!("reference_support.rs")),
        ] {
            assert!(!src.contains(&packages), "{path} must not use catalog {packages} as binding");
            assert!(!src.contains(&modules), "{path} must not use catalog {modules} as binding");
            assert!(
                !src.contains(&type_units),
                "{path} must not use catalog {type_units} as binding"
            );
            assert!(!src.contains(&candidates), "{path} must not use {candidates} as identity");
        }
    }

    /// Cold start of one file hits U1 / U2 / U3 once each. The three
    /// unexpanded parses stay split (empty vs profile predefines vs Trace);
    /// `preprocessor_independent` is one function on U1 and U2.
    #[test]
    fn cold_start_unexpanded_parse_count_matches_three_sites() {
        use base_db::{change::Change, source_root::SourceRoot};
        use preproc_expand::db::PreprocDb;
        use syntax::UNEXPANDED_PARSE_RUNS;
        use vfs::{ChangedFile, FileId, FileSet, VfsPath};

        let file_id = FileId::from_raw(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/top.sv".to_owned()));
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(file_id, "module top;\n  int w;\nendmodule\n"));
        let mut host = crate::analysis_host::AnalysisHost::default();
        UNEXPANDED_PARSE_RUNS.with(|runs| runs.set(0));
        host.apply_change_without_prewarm(change);

        let ctx = host.ctx();
        let db: &dyn PreprocDb = ctx.db;
        let _ = db.source_model(file_id);
        let _ = ctx.file_facts(file_id);
        let _ = db.compilation_plan_for_root(db.source_root_id(file_id));
        let runs = UNEXPANDED_PARSE_RUNS.with(Cell::get);
        assert_eq!(
            runs, 3,
            "cold start of one file must unexpanded-parse once per site (source_model, file_facts, include_scan); ran {runs}"
        );
    }
}
