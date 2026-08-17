use base_db::source_root::SourceRootId;
use hir_def::pathres::ResolutionContext;
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use super::{
    epoch::{EpochDecision, StructureEpoch, StructureSnapshot},
    indexes::{GenArc, ModuleEdgeEntry, ReferenceIndexEntry, file_gen},
    product_cell::ProductCell,
};
use crate::{
    analysis::AnalysisContext,
    db::root_db::RootDb,
    semantic_index::{FileSemanticIndex, ModuleEdgeIndex, ReferenceIndex, SemanticSnapshotInputs},
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
    pub reference_roots: FxHashSet<SourceRootId>,
}

#[derive(Clone, Default)]
struct StructureProducts {
    resolution: Arc<ProductCell<ResolutionContext>>,
    snapshot_inputs: Arc<ProductCell<SemanticSnapshotInputs>>,
}

#[derive(Clone, Default)]
struct Shards {
    file_indexes: FxHashMap<FileId, GenArc<FileSemanticIndex>>,
    module_edges: FxHashMap<SourceRootId, ModuleEdgeEntry>,
    references: FxHashMap<SourceRootId, ReferenceIndexEntry>,
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
}

impl Inner {
    fn drop_structure_products(&mut self) {
        self.structure.resolution = Arc::new(ProductCell::default());
        self.structure.snapshot_inputs = Arc::new(ProductCell::default());
        self.shards.file_indexes.clear();
        self.shards.module_edges.clear();
        self.shards.references.clear();
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

    /// Record the files made dirty by a change before Salsa applies it, so the
    /// pre-change structure snapshots can be compared against the post-change
    /// trees when the epoch is decided.
    pub(crate) fn capture_epoch(&self, db: &RootDb, files: &[FileId]) {
        if files.is_empty() {
            return;
        }
        // Capture pre-change snapshots outside the lock: Salsa queries must not
        // run while holding the store mutex.
        let capture_structure = self.inner.lock().structure.resolution.is_ready();
        let snapshots = if capture_structure {
            files
                .iter()
                .map(|&file_id| (file_id, StructureSnapshot::capture(db, file_id)))
                .collect()
        } else {
            Vec::new()
        };
        let mut inner = self.inner.lock();
        if snapshots.is_empty() {
            inner.epoch.mark_dirty(files);
        } else {
            inner.epoch.record(snapshots);
        }
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

    pub(crate) fn file_index(
        &self,
        ctx: &AnalysisContext<'_>,
        file_id: FileId,
    ) -> Arc<FileSemanticIndex> {
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

        let context = ctx.semantic_snapshot_inputs();
        let index = Arc::new(FileSemanticIndex::for_file_with_context(ctx.db, file_id, &context));
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
        let root_files = source_root_files(ctx, source_root_id);
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

    pub(crate) fn references(
        &self,
        ctx: &AnalysisContext<'_>,
        source_root_id: SourceRootId,
    ) -> Arc<ReferenceIndex> {
        let root_files = source_root_files(ctx, source_root_id);
        let (mut entry, gens) = {
            let mut inner = self.inner.lock();
            inner.hot.reference_roots.insert(source_root_id);
            if let Some(entry) = inner.shards.references.get(&source_root_id)
                && entry.is_fresh(&root_files, &inner.dirty_gen)
            {
                return entry.index.clone();
            }
            (
                inner.shards.references.get(&source_root_id).cloned().unwrap_or_default(),
                inner.dirty_gen.clone(),
            )
        };

        entry.refresh(ctx, &root_files, &gens);
        let result = entry.index.clone();
        self.inner.lock().shards.references.insert(source_root_id, entry);
        result
    }
}

fn source_root_files(ctx: &AnalysisContext<'_>, source_root_id: SourceRootId) -> Vec<FileId> {
    ctx.db.source_root(source_root_id).iter().collect()
}
