use base_db::source_root::SourceRootId;
use hir_def::pathres::ResolutionContext;
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use super::{
    epoch::{EpochDecision, StructureEpoch, StructureSnapshot},
    indexes::{GenArc, ModuleEdgeEntry, NameIndexEntry, file_gen},
    product_cell::ProductCell,
};
use crate::{
    analysis::AnalysisContext,
    db::root_db::RootDb,
    name_index::{FileNameIndex, NameIndex, index_files_for_root},
    semantic_index::{ModuleEdgeIndex, SemanticSnapshotInputs},
};

/// Products that have been requested at least once on this store lineage.
///
/// Survives [`EpochDecision::Drop`] so a structural edit still prewarms what
/// the user was using. Dies with the store on a workspace reset.
#[derive(Clone, Default)]
pub(crate) struct HotProducts {
    pub snapshot_inputs: bool,
    pub files: FxHashSet<FileId>,
    pub module_edge_roots: FxHashSet<SourceRootId>,
    pub name_index_roots: FxHashSet<SourceRootId>,
}

#[derive(Clone, Default)]
struct StructureProducts {
    resolution: Arc<ProductCell<ResolutionContext>>,
    snapshot_inputs: Arc<ProductCell<SemanticSnapshotInputs>>,
}

#[derive(Clone, Default)]
struct Shards {
    file_indexes: FxHashMap<FileId, GenArc<FileNameIndex>>,
    module_edges: FxHashMap<SourceRootId, ModuleEdgeEntry>,
    names: FxHashMap<SourceRootId, NameIndexEntry>,
}

#[derive(Clone, Default)]
struct Inner {
    epoch: StructureEpoch,
    /// How many times each file has been in an affected set since this store
    /// was created. A shard built at generation G is stale when `dirty_gen`
    /// has moved past G. Consecutive edits without a request accumulate here
    /// instead of replacing a single dirty set.
    dirty_gen: FxHashMap<FileId, u64>,
    structure: StructureProducts,
    shards: Shards,
    hot: HotProducts,
    /// An authoritative parse (or an explicit plan query) has used the
    /// include graph. Until then, a text edit cannot have invalidated an
    /// expanded CST, so dirty propagation must not start an include scan.
    include_graph_used: bool,
}

impl Inner {
    fn drop_structure_products(&mut self) {
        self.structure.resolution = Arc::new(ProductCell::default());
        self.structure.snapshot_inputs = Arc::new(ProductCell::default());
        self.shards.file_indexes.clear();
        self.shards.module_edges.clear();
        self.shards.names.clear();
    }
}

/// Lazily materialized workspace products, forked on every change so
/// previously created [`crate::analysis::AnalysisSnapshot`]s keep the previous
/// value and can never observe products from a later edit.
///
/// Owned by [`crate::analysis_host::AnalysisHost`].
#[derive(Default)]
pub(crate) struct ProductStore {
    inner: Mutex<Inner>,
}

impl std::panic::RefUnwindSafe for ProductStore {}
impl std::panic::UnwindSafe for ProductStore {}

impl std::fmt::Debug for ProductStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProductStore").finish()
    }
}

impl ProductStore {
    pub(crate) fn fork(&self) -> Self {
        Self { inner: Mutex::new(self.inner.lock().clone()) }
    }

    pub(crate) fn hot(&self) -> HotProducts {
        self.inner.lock().hot.clone()
    }

    pub(crate) fn mark_include_graph_used(&self) {
        self.inner.lock().include_graph_used = true;
    }

    pub(crate) fn include_graph_used(&self) -> bool {
        self.inner.lock().include_graph_used
    }

    /// Record the files made dirty by a change before Salsa applies it, so the
    /// pre-change structure snapshots can be compared against the post-change
    /// trees when the epoch is decided.
    pub(crate) fn capture_epoch(&self, db: &RootDb, files: &[FileId]) {
        if files.is_empty() {
            return;
        }
        // Capture pre-change L0 shards outside the lock: Salsa queries must
        // not run while holding the store mutex. Always snapshot — name
        // tables do not depend on resolution being warm, and a missing
        // snapshot cannot prove a body-only edit.
        let snapshots: Vec<_> = files
            .iter()
            .map(|&file_id| (file_id, StructureSnapshot::capture(db, file_id)))
            .collect();
        self.inner.lock().epoch.record(snapshots);
    }

    /// Apply the structural epoch. Body-only edits keep the previous
    /// resolution products; structural edits discard them before any IDE
    /// request observes the new store. The per-file generation clock always
    /// advances for the affected set.
    ///
    /// This is the only invalidation entry point. The request path never
    /// re-decides the epoch.
    pub(crate) fn invalidate(&self, db: &RootDb, files: &[FileId]) {
        let epoch = self.inner.lock().epoch.clone();
        let decision = if epoch.is_empty() { EpochDecision::Keep } else { epoch.decide(db) };
        let mut inner = self.inner.lock();
        inner.epoch.clear();
        for &file_id in files {
            *inner.dirty_gen.entry(file_id).or_insert(0) += 1;
        }
        if decision == EpochDecision::Drop {
            inner.drop_structure_products();
        }
    }

    pub(crate) fn resolution_cell(&self) -> Arc<ProductCell<ResolutionContext>> {
        self.inner.lock().structure.resolution.clone()
    }

    pub(crate) fn snapshot_inputs_cell(&self) -> Arc<ProductCell<SemanticSnapshotInputs>> {
        let mut inner = self.inner.lock();
        inner.hot.snapshot_inputs = true;
        inner.structure.snapshot_inputs.clone()
    }

    pub(crate) fn file_name_index(
        &self,
        ctx: &AnalysisContext<'_>,
        file_id: FileId,
    ) -> Arc<FileNameIndex> {
        let current_gen = {
            let mut inner = self.inner.lock();
            inner.hot.files.insert(file_id);
            let generation = file_gen(&inner.dirty_gen, file_id);
            if let Some(shard) = inner.shards.file_indexes.get(&file_id)
                && shard.built_gen == generation
            {
                return shard.value.clone();
            }
            generation
        };

        let index = Arc::new(FileNameIndex::for_file(ctx.db, file_id));
        let mut inner = self.inner.lock();
        inner
            .shards
            .file_indexes
            .insert(file_id, GenArc { value: index.clone(), built_gen: current_gen });
        index
    }

    pub(crate) fn module_edges(
        &self,
        ctx: &AnalysisContext<'_>,
        source_root_id: SourceRootId,
    ) -> Arc<ModuleEdgeIndex> {
        let root_files = index_files_for_root(ctx, source_root_id);
        let (mut entry, gens) = {
            let mut inner = self.inner.lock();
            inner.hot.module_edge_roots.insert(source_root_id);
            if let Some(entry) = inner.shards.module_edges.get(&source_root_id)
                && entry.is_fresh(&root_files, &inner.dirty_gen)
            {
                return entry.index.clone();
            }
            (
                inner.shards.module_edges.get(&source_root_id).cloned().unwrap_or_default(),
                inner.dirty_gen.clone(),
            )
        };

        entry.refresh(ctx, &root_files, &gens);
        let result = entry.index.clone();
        self.inner.lock().shards.module_edges.insert(source_root_id, entry);
        result
    }

    pub(crate) fn name_index(
        &self,
        ctx: &AnalysisContext<'_>,
        source_root_id: SourceRootId,
    ) -> Arc<NameIndex> {
        let root_files = index_files_for_root(ctx, source_root_id);
        let (mut entry, gens) = {
            let mut inner = self.inner.lock();
            inner.hot.name_index_roots.insert(source_root_id);
            if let Some(entry) = inner.shards.names.get(&source_root_id)
                && entry.is_fresh(&root_files, &inner.dirty_gen)
            {
                return entry.index.clone();
            }
            (
                inner.shards.names.get(&source_root_id).cloned().unwrap_or_default(),
                inner.dirty_gen.clone(),
            )
        };

        entry.refresh(ctx, &root_files, &gens);
        let result = entry.index.clone();
        self.inner.lock().shards.names.insert(source_root_id, entry);
        result
    }
}
