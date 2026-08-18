use std::{
    ops::{Deref, Range},
    sync::atomic::AtomicBool,
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
use preproc_expand::{compilation_plan::CompilationPlan, profile_compiler::ProfileCompilationJob};
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
    completion::{CompletionItem, context::TriggerChar},
    db::{line_index_db::LineIndexDb, root_db::RootDb},
    diagnostics,
    document_highlight::{self, DocumentHighlight, DocumentHighlightConfig},
    document_symbols::{self, DocumentSymbol},
    folding_ranges::{self, Fold},
    formatting::{self, FmtConfig},
    goto_declaration, goto_definition, hover,
    incrementality::{ComputationPriority, ProductStore},
    inlay_hint::{self, InlayHint, InlayHintConfig},
    markup::Markup,
    navigation_target::NavTarget,
    references::{self, References, ReferencesConfig},
    rename::{self, RenameConfig, RenameResult},
    selection_ranges,
    semantic_index::{self, ModuleCallEdge},
    semantic_tokens::{self, SemaToken, SemaTokenConfig},
    signature_help::{self, SignatureHelp, SignatureHelpConfig},
    source_change::SourceChange,
    workspace_symbols::{self, WorkspaceSymbol},
};

#[derive(Debug)]
pub struct AnalysisSnapshot {
    pub(crate) db: RootDb,
    pub(crate) store: Arc<ProductStore>,
    pub(crate) snapshot_id: AnalysisSnapshotId,
    pub(crate) salsa_revision: base_db::salsa::Revision,
}

static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Read view of one IDE request: the pure Salsa database plus the
/// workspace product store. Features are pure functions of this context,
/// so they can never observe products from a later edit.
pub(crate) struct AnalysisContext<'a> {
    pub(crate) db: &'a RootDb,
    pub(crate) store: &'a ProductStore,
}

impl Deref for AnalysisContext<'_> {
    type Target = RootDb;

    fn deref(&self) -> &RootDb {
        self.db
    }
}

impl AnalysisContext<'_> {
    pub(crate) fn new<'a>(db: &'a RootDb, store: &'a ProductStore) -> AnalysisContext<'a> {
        AnalysisContext { db, store }
    }

    pub(crate) fn semantics(&self) -> hir_semantics::semantics::Semantics<'_, RootDb> {
        hir_semantics::semantics::Semantics::new_with_context(self.db, self.resolution())
    }

    /// Parse one file without building `$unit` or the design map.
    pub(crate) fn parse_file(&self, file_id: FileId) -> syntax::SyntaxTree {
        let (tree, dependencies) = self.db.parse_src_with_dependencies(file_id);
        self.store.record_parse_dependencies(file_id, dependencies);
        crate::generated_units::record_from_paid_artifact(self, file_id);
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

    pub(crate) fn design_graph(&self) -> triomphe::Arc<design_graph::DesignGraph> {
        self.design_graph_with_priority(crate::incrementality::ComputationPriority::Foreground)
            .expect("foreground design-graph fold cannot be cancelled")
    }

    pub(crate) fn prewarm_design_graph(
        &self,
        cancel: &AtomicBool,
    ) -> Option<triomphe::Arc<design_graph::DesignGraph>> {
        self.design_graph_with_priority_cancel(
            crate::incrementality::ComputationPriority::Background,
            cancel,
        )
    }

    pub(crate) fn prewarm_resolution(&self, cancel: &AtomicBool) -> Option<Arc<ResolutionContext>> {
        self.resolution_with_priority(ComputationPriority::Background, cancel)
    }

    fn design_graph_with_priority(
        &self,
        priority: crate::incrementality::ComputationPriority,
    ) -> Option<triomphe::Arc<design_graph::DesignGraph>> {
        self.design_graph_with_priority_cancel(priority, &NEVER_CANCELLED)
    }

    fn design_graph_with_priority_cancel(
        &self,
        priority: crate::incrementality::ComputationPriority,
        cancel: &AtomicBool,
    ) -> Option<triomphe::Arc<design_graph::DesignGraph>> {
        let generated = self.store.generated_units();
        self.store.design_graph_cell().get_or_compute(priority, cancel, |in_flight| {
            let _span = tracing::info_span!("design_graph.build").entered();
            let started = std::time::Instant::now();
            let files: Vec<_> = self
                .db
                .files()
                .iter()
                .copied()
                .filter(|&file_id| self.db.file_kind(file_id).is_semantic_compilation_unit())
                .collect();
            let Some(facts) = file_facts_parallel(self.db, &files, cancel, in_flight) else {
                return triomphe::Arc::new(design_graph::DesignGraph::default());
            };
            let graph = design_graph::DesignGraph::from_file_facts(
                facts.iter().map(std::convert::AsRef::as_ref),
                &generated,
            );
            let file_count = facts.len();
            let independent_files =
                facts.iter().filter(|facts| facts.preprocessor_independent).count();
            tracing::info!(
                file_count,
                node_count = graph.node_count(),
                generated_node_count = generated.meta.len(),
                independent_files,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "design_graph.build"
            );
            triomphe::Arc::new(graph)
        })
    }

    pub(crate) fn resolution(&self) -> Arc<ResolutionContext> {
        self.resolution_with_priority(ComputationPriority::Foreground, &NEVER_CANCELLED)
            .expect("foreground resolution computation cannot be cancelled")
    }

    fn resolution_with_priority(
        &self,
        priority: ComputationPriority,
        cancel: &AtomicBool,
    ) -> Option<Arc<ResolutionContext>> {
        self.store.resolution_cell().get_or_compute(priority, cancel, |_| {
            ResolutionContext::from_graph(self.design_graph())
        })
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

/// Unexpanded `file_facts` are independent per file. Folding them sequentially
/// is the ready-path cost on a library-sized workspace.
fn file_facts_parallel(
    db: &RootDb,
    files: &[FileId],
    cancel_a: &AtomicBool,
    cancel_b: &AtomicBool,
) -> Option<Vec<Arc<design_graph::FileFacts>>> {
    let cancelled = || {
        cancel_a.load(std::sync::atomic::Ordering::Acquire)
            || cancel_b.load(std::sync::atomic::Ordering::Acquire)
    };
    if cancelled() {
        return None;
    }
    let threads =
        std::thread::available_parallelism().map(usize::from).unwrap_or(1).min(files.len());
    if threads <= 1 {
        let mut facts = Vec::with_capacity(files.len());
        for &file_id in files {
            if cancelled() {
                return None;
            }
            facts.push(<dyn DesignGraphDb>::file_facts(db, file_id));
        }
        return Some(facts);
    }

    let chunk_size = files.len().div_ceil(threads);
    let stop = std::sync::atomic::AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for chunk in files.chunks(chunk_size) {
            let chunk: Vec<FileId> = chunk.to_vec();
            let db = db.clone();
            let cancel_a = cancel_a;
            let cancel_b = cancel_b;
            let stop = &stop;
            handles.push(scope.spawn(move || {
                let mut facts = Vec::with_capacity(chunk.len());
                for file_id in chunk {
                    if cancel_a.load(std::sync::atomic::Ordering::Acquire)
                        || cancel_b.load(std::sync::atomic::Ordering::Acquire)
                        || stop.load(std::sync::atomic::Ordering::Acquire)
                    {
                        stop.store(true, std::sync::atomic::Ordering::Release);
                        return None;
                    }
                    facts.push(<dyn DesignGraphDb>::file_facts(&db, file_id));
                }
                Some(facts)
            }));
        }
        let mut facts = Vec::with_capacity(files.len());
        for handle in handles {
            facts.extend(handle.join().expect("file_facts worker")?);
        }
        Some(facts)
    });
    result.filter(|_| !cancelled())
}

impl AnalysisSnapshot {
    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
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
        let ctx = AnalysisContext::new(&self.db, &self.store);
        Cancelled::catch(|| f(&ctx))
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

    pub fn compilation_profile_job(
        &self,
        profile_id: CompilationProfileId,
    ) -> Cancellable<ProfileCompilationJob> {
        self.with_db(|db| {
            preproc_expand::profile_compiler::build_profile_compilation_job(db.db, profile_id)
        })
    }

    pub fn file_vide_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::vide_diagnostics(db.db, db.resolution().as_ref(), file_id))
    }

    pub fn parse_diagnostics(&self, file_id: FileId) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::parse_diagnostics(db, file_id))
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
        self.with_db(|db| semantic_index::incoming_module_edges(db, file_id, name_range))
    }

    pub fn module_outgoing_calls(
        &self,
        file_id: FileId,
        name_range: TextRange,
    ) -> Cancellable<Vec<ModuleCallEdge>> {
        self.with_db(|db| semantic_index::outgoing_module_edges(db, file_id, name_range))
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
            inlay_hint::inlay_hint(db, db.design_graph().as_ref(), file_id, range, config)
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
