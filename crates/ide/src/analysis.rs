use std::{
    ops::{Deref, Range},
    sync::atomic::AtomicBool,
};

use base_db::{
    Cancelled,
    analysis_snapshot::AnalysisSnapshotId,
    project::CompilationProfileId,
    salsa,
    source_db::{SourceDb, SourceRootDb},
    source_root::{SourceRootId, SourceRootRole},
};
use hir_def::{def_id::DefId, pathres::ResolutionContext};
use preproc_expand::{compilation_plan::CompilationPlan, file::HirFileId};
use rustc_hash::FxHashMap;
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
    inlay_hint::{self, InlayHint, InlayHintConfig},
    markup::Markup,
    navigation_target::NavTarget,
    references::{self, References, ReferencesConfig},
    rename::{self, RenameConfig, RenameResult},
    revision_cache::{ComputationPriority, RevisionCache},
    selection_ranges,
    semantic_index::{
        self, FileModuleEdges, FileSemanticIndex, ModuleCallEdge, ModuleEdgeIndex, ReferenceIndex,
        SemanticSnapshotInputs,
    },
    semantic_tokens::{self, SemaToken, SemaTokenConfig},
    signature_help::{self, SignatureHelp, SignatureHelpConfig},
    source_change::SourceChange,
    workspace_symbols::{self, WorkspaceSymbol},
};

#[derive(Debug)]
pub struct AnalysisSnapshot {
    pub(crate) db: RootDb,
    pub(crate) cache: Arc<RevisionCache>,
    pub(crate) snapshot_id: AnalysisSnapshotId,
    pub(crate) salsa_revision: base_db::salsa::Revision,
}

static NEVER_CANCELLED: AtomicBool = AtomicBool::new(false);

/// Read view of one IDE request: the pure Salsa database plus the
/// revision-scoped workspace cache. Features are pure functions of this
/// context, so they can never observe products from a later edit.
pub(crate) struct AnalysisContext<'a> {
    pub(crate) db: &'a RootDb,
    pub(crate) cache: &'a RevisionCache,
}

impl Deref for AnalysisContext<'_> {
    type Target = RootDb;

    fn deref(&self) -> &RootDb {
        self.db
    }
}

impl AnalysisContext<'_> {
    pub(crate) fn new<'a>(db: &'a RootDb, cache: &'a RevisionCache) -> AnalysisContext<'a> {
        AnalysisContext { db, cache }
    }

    pub(crate) fn semantics(&self) -> hir_semantics::semantics::Semantics<'_, RootDb> {
        hir_semantics::semantics::Semantics::new_with_context(
            self.db,
            self.request_hir_resolution_context(),
        )
    }

    pub(crate) fn has_materialized_semantic_inputs(&self) -> bool {
        self.cache.lock().revision.semantic_inputs.is_ready()
    }

    pub(crate) fn has_materialized_file_index(&self, file_id: FileId) -> bool {
        self.cache.lock().indexes.request_file_indexes.contains_key(&file_id)
    }

    pub(crate) fn has_materialized_module_edges(&self, root: SourceRootId) -> bool {
        self.cache.lock().indexes.module_edge_entries.contains_key(&root)
    }

    pub(crate) fn has_materialized_reference_index(&self, root: SourceRootId) -> bool {
        self.cache.lock().indexes.reference_entries.contains_key(&root)
    }

    pub(crate) fn request_source_semantic_map(
        &self,
        file_id: FileId,
    ) -> Arc<preproc_expand::macro_file::SourceSemanticMap> {
        if let Some(map) = self.cache.lock().indexes.source_semantic_maps.get(&file_id).cloned() {
            return map;
        }
        let map = self.db.source_semantic_map(file_id);
        self.cache.lock().indexes.source_semantic_maps.insert(file_id, map.clone());
        map
    }

    pub(crate) fn request_unit_index(&self) -> Arc<hir_def::unit_index::UnitIndex> {
        self.request_hir_resolution_context().unit_index()
    }

    pub(crate) fn request_module_index(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<crate::semantic_index::ModuleIndex> {
        self.semantic_snapshot_inputs().module_index(source_root_id).unwrap_or_default()
    }

    pub(crate) fn request_module_edge_index(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<ModuleEdgeIndex> {
        let context = self.semantic_snapshot_inputs();
        let revision = salsa::plumbing::current_revision(self.db);
        let (dirty, mut entry) = {
            let cache = self.cache.lock();
            let entry =
                cache.indexes.module_edge_entries.get(&source_root_id).cloned().unwrap_or_default();
            if entry.built_at == Some(revision) {
                return entry.index;
            }
            (cache.indexes.module_edge_dirty.clone(), entry)
        };

        let source_root = self.db.source_root(source_root_id);
        let needs_full = dirty.is_empty() || entry.file_edges.is_empty();
        if needs_full {
            entry.file_edges = source_root
                .iter()
                .map(|file_id| {
                    (
                        file_id,
                        Arc::new(FileModuleEdges::for_file_with_indexes(
                            self.db,
                            file_id,
                            context.module_indexes(),
                        )),
                    )
                })
                .collect();
        } else {
            for file_id in dirty {
                if source_root.iter().any(|candidate| candidate == file_id) {
                    entry.file_edges.insert(
                        file_id,
                        Arc::new(FileModuleEdges::for_file_with_indexes(
                            self.db,
                            file_id,
                            context.module_indexes(),
                        )),
                    );
                }
            }
        }
        entry.index =
            Arc::new(ModuleEdgeIndex::from_file_edges(entry.file_edges.values().map(Arc::as_ref)));
        entry.built_at = Some(revision);
        let result = entry.index.clone();
        let mut cache = self.cache.lock();
        let stored = cache.indexes.module_edge_entries.entry(source_root_id).or_default();
        if stored.built_at != Some(revision) {
            *stored = entry;
        }
        result
    }

    pub(crate) fn semantic_snapshot_inputs(&self) -> Arc<SemanticSnapshotInputs> {
        self.semantic_snapshot_inputs_with_priority(
            ComputationPriority::Foreground,
            &NEVER_CANCELLED,
        )
        .expect("foreground semantic input computation cannot be cancelled")
    }

    pub(crate) fn prewarm_semantic_snapshot_inputs(
        &self,
        cancel: &AtomicBool,
    ) -> Option<Arc<SemanticSnapshotInputs>> {
        self.semantic_snapshot_inputs_with_priority(ComputationPriority::Background, cancel)
    }

    fn semantic_snapshot_inputs_with_priority(
        &self,
        priority: ComputationPriority,
        cancel: &AtomicBool,
    ) -> Option<Arc<SemanticSnapshotInputs>> {
        let hir = self.request_hir_resolution_context_with_priority(priority, cancel)?;
        let cell = self.cache.lock().revision.semantic_inputs.clone();
        cell.get_or_compute(priority, cancel, |_| {
            crate::semantic_index::SemanticSnapshotInputs::from_db_with_hir(self.db, hir)
        })
    }

    pub(crate) fn request_file_semantic_index(&self, file_id: FileId) -> Arc<FileSemanticIndex> {
        let context = self.semantic_snapshot_inputs();
        {
            let cache = self.cache.lock();
            if !cache.indexes.request_file_index_dirty.contains(&file_id)
                && let Some(index) = cache.indexes.request_file_indexes.get(&file_id)
            {
                return index.clone();
            }
        }

        let index = Arc::new(FileSemanticIndex::for_file_with_context(self.db, file_id, &context));
        let mut cache = self.cache.lock();
        cache.indexes.request_file_indexes.insert(file_id, index.clone());
        cache.indexes.request_file_index_dirty.remove(&file_id);
        index
    }

    fn request_hir_resolution_context(&self) -> Arc<ResolutionContext> {
        self.request_hir_resolution_context_with_priority(
            ComputationPriority::Foreground,
            &NEVER_CANCELLED,
        )
        .expect("foreground resolution computation cannot be cancelled")
    }

    fn request_hir_resolution_context_with_priority(
        &self,
        priority: ComputationPriority,
        cancel: &AtomicBool,
    ) -> Option<Arc<ResolutionContext>> {
        let revision = salsa::plumbing::current_revision(self.db);
        let (built_at, ready, epoch) = {
            let cache = self.cache.lock();
            (
                cache.revision.resolution_built_at,
                cache.revision.hir_resolution_context.is_ready(),
                cache.revision.structure_epoch.clone(),
            )
        };
        if built_at != Some(revision) {
            let needs_rebuild = !ready || epoch.is_empty() || !epoch.reusable(self.db);
            let mut cache = self.cache.lock();
            if cache.revision.resolution_built_at != Some(revision) {
                cache.revision.structure_epoch.clear();
                if needs_rebuild {
                    cache.discard_resolution_products();
                }
                cache.revision.resolution_built_at = Some(revision);
            }
        }
        let cell = self.cache.lock().revision.hir_resolution_context.clone();
        cell.get_or_compute(priority, cancel, |_| ResolutionContext::from_db(self.db))
    }

    pub(crate) fn reference_index_for_root(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<ReferenceIndex> {
        let revision = salsa::plumbing::current_revision(self.db);
        let (dirty, mut entry) = {
            let cache = self.cache.lock();
            let entry =
                cache.indexes.reference_entries.get(&source_root_id).cloned().unwrap_or_default();
            if entry.built_at == Some(revision) {
                return entry.index;
            }
            (cache.indexes.reference_dirty.clone(), entry)
        };

        let current_files = self.db.files();

        // A structural change (or first build) forces a full rebuild, because a
        // changed definition can affect name resolution in every other file.
        let needs_full = dirty.is_empty()
            || entry.file_indexes.is_empty()
            || dirty.iter().any(|file_id| {
                !current_files.contains(file_id)
                    || entry
                        .item_trees
                        .get(file_id)
                        .map_or(true, |old| *old != self.db.item_tree(HirFileId::File(*file_id)))
            });
        if needs_full {
            let context = self.semantic_snapshot_inputs();
            let mut file_indexes = FxHashMap::default();
            let mut item_trees = FxHashMap::default();
            for file_id in self.db.source_root(source_root_id).iter() {
                file_indexes.insert(
                    file_id,
                    Arc::new(crate::semantic_index::FileSemanticIndex::for_file_with_context(
                        self.db, file_id, &context,
                    )),
                );
                item_trees.insert(file_id, self.db.item_tree(HirFileId::File(file_id)));
            }
            entry.index = Arc::new(ReferenceIndex::from_file_indexes(self.db, &file_indexes));
            entry.file_indexes = file_indexes;
            entry.item_trees = item_trees;
            entry.context = Some(context);
            entry.built_at = Some(revision);
        } else {
            // Incremental: patch the cached index with each dirty file's new
            // contribution, reusing cached name/ranges for existing definitions.
            for file_id in &dirty {
                let old_file_index = entry.file_indexes.get(file_id).cloned().unwrap_or_default();
                let new_file_index =
                    Arc::new(crate::semantic_index::FileSemanticIndex::for_file_with_context(
                        self.db,
                        *file_id,
                        entry.context.as_ref().unwrap(),
                    ));
                Arc::make_mut(&mut entry.index).patch_file(
                    self.db,
                    *file_id,
                    &old_file_index,
                    &new_file_index,
                );
                entry.file_indexes.insert(*file_id, new_file_index);
                entry.item_trees.insert(*file_id, self.db.item_tree(HirFileId::File(*file_id)));
            }
            entry.built_at = Some(revision);
        }
        let result = entry.index.clone();
        let mut cache = self.cache.lock();
        let stored = cache.indexes.reference_entries.entry(source_root_id).or_default();
        if stored.built_at != Some(revision) {
            *stored = entry;
        }
        result
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
        let ctx = AnalysisContext::new(&self.db, &self.cache);
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
        self.with_db(|db| diagnostics::diagnostics(db, file_id))
    }

    pub fn compilation_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::compilation_diagnostics(db, file_id))
    }

    pub fn source_root_diagnostics(
        &self,
        file_id: FileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::source_root_diagnostics(db, file_id))
    }

    pub fn compilation_profile_diagnostics(
        &self,
        profile_id: CompilationProfileId,
    ) -> Cancellable<Vec<diagnostics::Diagnostic>> {
        self.with_db(|db| diagnostics::compilation_profile_diagnostics(db, profile_id))
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
        self.with_db(|db| db.compilation_plan_for_root(db.source_root_id(file_id)))
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
        self.with_db(|db| inlay_hint::inlay_hint(db, file_id, range, config))
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
