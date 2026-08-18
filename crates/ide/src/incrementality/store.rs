use base_db::{source_db::SourceDb, source_root::SourceRootId};
use design_graph::{DesignGraph, DesignGraphDb, GeneratedUnits, UnitId, UnitMeta};
use hir_def::pathres::ResolutionContext;
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use super::{
    epoch::{EpochDecision, StructureEpoch, StructureSnapshot},
    indexes::{GenArc, NameIndexEntry, file_gen},
    product_cell::ProductCell,
};
use crate::{
    analysis::AnalysisContext,
    db::root_db::RootDb,
    name_index::{FileNameIndex, NameIndex, index_files_for_root},
    semantic_index::SemanticSnapshotInputs,
};

/// Products that have been requested at least once on this store lineage.
///
/// Survives a structure [`EpochDecision::Patch`] so a CU edit still prewarms
/// what the user was using. Dies with the store on a workspace reset.
#[derive(Clone)]
pub(crate) struct HotProducts {
    pub snapshot_inputs: bool,
    /// Always a workspace product. True from initialize so ready waits for
    /// fold.
    pub design_graph: bool,
    pub files: FxHashSet<FileId>,
    pub name_index_roots: FxHashSet<SourceRootId>,
}

impl Default for HotProducts {
    fn default() -> Self {
        Self {
            snapshot_inputs: false,
            design_graph: true,
            files: FxHashSet::default(),
            name_index_roots: FxHashSet::default(),
        }
    }
}

#[derive(Clone, Default)]
struct StructureProducts {
    design_graph: Arc<ProductCell<DesignGraph>>,
    resolution: Arc<ProductCell<ResolutionContext>>,
    snapshot_inputs: Arc<ProductCell<SemanticSnapshotInputs>>,
}

#[derive(Clone, Default)]
struct Shards {
    file_indexes: FxHashMap<FileId, GenArc<FileNameIndex>>,
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
    /// Authoritative standalone parses retained by this store lineage:
    /// compilation root -> files named by emitted preprocessor include edges.
    parse_dependencies: FxHashMap<FileId, Arc<[FileId]>>,
    /// Generated CU units from paid artifacts. Write-only in this PR.
    generated: GeneratedUnits,
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

    pub(crate) fn record_parse_dependencies(&self, file_id: FileId, dependencies: Arc<[FileId]>) {
        self.inner.lock().parse_dependencies.insert(file_id, dependencies);
    }

    /// Book-keep generated units for one file. Returns whether the stored set
    /// changed so the caller can upsert that file on the live graph.
    pub(crate) fn record_generated_units(
        &self,
        file_id: FileId,
        ids: Box<[UnitId]>,
        meta: FxHashMap<UnitId, UnitMeta>,
    ) -> bool {
        self.inner.lock().generated.replace_file(file_id, ids, meta)
    }

    pub(crate) fn generated_units(&self) -> GeneratedUnits {
        self.inner.lock().generated.clone()
    }

    pub(crate) fn design_graph_cell(&self) -> Arc<ProductCell<DesignGraph>> {
        let mut inner = self.inner.lock();
        inner.hot.design_graph = true;
        inner.structure.design_graph.clone()
    }

    pub(crate) fn parsed_dependents(&self, changed: &[FileId]) -> Vec<FileId> {
        let changed = changed.iter().copied().collect::<FxHashSet<_>>();
        self.inner
            .lock()
            .parse_dependencies
            .iter()
            .filter_map(|(&file_id, dependencies)| {
                (!changed.contains(&file_id)
                    && dependencies.iter().any(|dependency| changed.contains(dependency)))
                .then_some(file_id)
            })
            .collect()
    }

    /// Record the files made dirty by a change before Salsa applies it, so the
    /// pre-change structure snapshots can be compared against the post-change
    /// trees when the epoch is decided.
    pub(crate) fn capture_epoch(&self, db: &RootDb, files: &[FileId]) {
        if files.is_empty() {
            return;
        }
        // Capture pre-change L0 shards outside the lock: Salsa queries must
        // not run while holding the store mutex. Only snapshot files that
        // already exist — a create has no pre-change facts, and parsing an
        // empty slot is not a snapshot.
        let snapshots: Vec<_> = files
            .iter()
            .copied()
            .filter(|&file_id| db.files().contains(&file_id))
            .map(|file_id| (file_id, StructureSnapshot::capture(db, file_id)))
            .collect();
        let mut inner = self.inner.lock();
        inner.epoch.record(snapshots);
        inner.epoch.mark_dirty(files);
    }

    pub(crate) fn mark_epoch_dirty(&self, files: &[FileId]) {
        self.inner.lock().epoch.mark_dirty(files);
    }

    /// Apply the structural epoch. Body-only edits keep the previous
    /// graph; files whose CU units changed are upserted. Resolution products
    /// drop only when the graph actually changed. The per-file generation
    /// clock always advances for the affected set.
    ///
    /// This is the only invalidation entry point. The request path never
    /// re-decides the epoch.
    pub(crate) fn invalidate(&self, db: &RootDb, files: &[FileId]) {
        let epoch = self.inner.lock().epoch.clone();
        let decision = if epoch.is_empty() { EpochDecision::Keep } else { epoch.decide(db) };
        {
            let mut inner = self.inner.lock();
            inner.epoch.clear();
            for &file_id in files {
                *inner.dirty_gen.entry(file_id).or_insert(0) += 1;
            }
        }
        match decision {
            EpochDecision::Keep => {}
            EpochDecision::Patch(patch) => {
                self.patch_design_graph(db, &patch);
                let mut inner = self.inner.lock();
                inner.structure.resolution = Arc::new(ProductCell::default());
                inner.structure.snapshot_inputs = Arc::new(ProductCell::default());
            }
        }
    }

    /// Upsert or remove `files` on the live graph. If the graph has never
    /// been built, leave the cell empty so the next request folds what exists.
    pub(crate) fn patch_design_graph(&self, db: &RootDb, files: &[FileId]) {
        if files.is_empty() {
            return;
        }
        let Some(current) = self.design_graph_cell().peek() else {
            return;
        };
        let generated = self.generated_units();
        let mut graph = (*current).clone();
        let mut changed = false;
        for &file_id in files {
            if !db.files().contains(&file_id)
                || !db.file_kind(file_id).is_semantic_compilation_unit()
            {
                changed |= graph.remove_file(file_id);
                continue;
            }
            changed |= graph.upsert_file(
                file_id,
                <dyn DesignGraphDb>::file_facts(db, file_id).as_ref(),
                &generated,
            );
        }
        if !changed {
            return;
        }
        let mut inner = self.inner.lock();
        inner.structure.design_graph = Arc::new(ProductCell::from_arc(triomphe::Arc::new(graph)));
        inner.structure.resolution = Arc::new(ProductCell::default());
        inner.structure.snapshot_inputs = Arc::new(ProductCell::default());
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
