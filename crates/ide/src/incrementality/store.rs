use base_db::source_db::SourceDb;
use design_graph::{DesignGraph, DesignGraphDb, GeneratedUnits, UnitId, UnitMeta};
use hir_def::pathres::ResolutionContext;
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use super::{
    epoch::{EpochDecision, StructureEpoch, StructureSnapshot},
    product_cell::ProductCell,
};
use crate::db::root_db::RootDb;

/// Products that have been requested at least once on this store lineage.
///
/// Survives a structure [`EpochDecision::Patch`] so a CU edit still prewarms
/// what the user was using. Dies with the store on a workspace reset.
#[derive(Clone)]
pub(crate) struct HotProducts {
    /// Always a workspace product. True from initialize so ready waits for
    /// fold.
    pub design_graph: bool,
}

impl Default for HotProducts {
    fn default() -> Self {
        Self { design_graph: true }
    }
}

#[derive(Clone, Default)]
struct StructureProducts {
    design_graph: Arc<ProductCell<DesignGraph>>,
    resolution: Arc<ProductCell<ResolutionContext>>,
}

#[derive(Clone, Default)]
struct Inner {
    epoch: StructureEpoch,
    structure: StructureProducts,
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
    /// drop only when the graph actually changed.
    ///
    /// This is the only invalidation entry point. The request path never
    /// re-decides the epoch.
    pub(crate) fn invalidate(&self, db: &RootDb, _files: &[FileId]) {
        let epoch = self.inner.lock().epoch.clone();
        let decision = if epoch.is_empty() { EpochDecision::Keep } else { epoch.decide(db) };
        self.inner.lock().epoch.clear();
        match decision {
            EpochDecision::Keep => {}
            EpochDecision::Patch(patch) => {
                self.patch_design_graph(db, &patch);
                let mut inner = self.inner.lock();
                inner.structure.resolution = Arc::new(ProductCell::default());
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
    }

    pub(crate) fn resolution_cell(&self) -> Arc<ProductCell<ResolutionContext>> {
        self.inner.lock().structure.resolution.clone()
    }
}
