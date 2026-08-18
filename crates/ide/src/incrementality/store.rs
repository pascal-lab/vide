use base_db::source_db::SourceDb;
use design_graph::{UnitCatalog, DesignGraphDb, GeneratedUnits, UnitId, UnitMeta};
use hir_def::pathres::ResolutionContext;
use parking_lot::Mutex;
use preproc_expand::db::PreprocDb;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use super::{
    epoch::{EpochDecision, StructureEpoch, StructureSnapshot},
    product_cell::ProductCell,
};
use crate::db::root_db::RootDb;

#[derive(Clone, Default)]
struct StructureProducts {
    design_graph: Arc<ProductCell<UnitCatalog>>,
    resolution: Arc<ProductCell<ResolutionContext>>,
}

#[derive(Clone, Default)]
struct Inner {
    epoch: StructureEpoch,
    structure: StructureProducts,
    /// Authoritative standalone parses retained by this store lineage:
    /// compilation root -> files named by emitted preprocessor include edges.
    parse_dependencies: FxHashMap<FileId, Arc<[FileId]>>,
    /// Generated CU units from paid artifacts, keyed by artifact fingerprint.
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

    pub(crate) fn record_parse_dependencies(&self, file_id: FileId, dependencies: Arc<[FileId]>) {
        self.inner.lock().parse_dependencies.insert(file_id, dependencies);
    }

    /// Book-keep generated units for one paid artifact. `fingerprint` is
    /// [`PreprocDb::compilation_unit_snapshot`]; a later snapshot with a
    /// different fingerprint cannot observe this entry.
    pub(crate) fn record_generated_units(
        &self,
        file_id: FileId,
        fingerprint: u64,
        ids: Box<[UnitId]>,
        meta: FxHashMap<UnitId, UnitMeta>,
    ) -> bool {
        self.inner.lock().generated.replace_file(file_id, fingerprint, ids, meta)
    }

    /// Generated units whose stored fingerprint still matches the current
    /// compilation-unit snapshot. Stale entries are a miss, not a value.
    pub(crate) fn generated_units(&self, db: &RootDb) -> GeneratedUnits {
        let mut generated = self.inner.lock().generated.clone();
        generated.retain_current(|file, fingerprint| {
            db.files().contains(&file)
                && <dyn PreprocDb>::compilation_unit_snapshot(db, file).fingerprint == fingerprint
        });
        generated
    }

    pub(crate) fn unit_catalog_cell(&self) -> Arc<ProductCell<UnitCatalog>> {
        self.inner.lock().structure.design_graph.clone()
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
    /// This is the only epoch-decision entry point. The request path may
    /// publish newly paid generated units onto an already-decided graph, but
    /// it never re-decides Keep vs Patch. Overlay entries whose artifact
    /// fingerprint no longer matches are dropped here so a Keep cannot retain
    /// a generated name the current snapshot cannot produce.
    pub(crate) fn invalidate(&self, db: &RootDb, _files: &[FileId]) {
        let stale_generated = self.drop_stale_generated(db);
        let epoch = self.inner.lock().epoch.clone();
        let decision = if epoch.is_empty() { EpochDecision::Keep } else { epoch.decide(db) };
        self.inner.lock().epoch.clear();
        let mut patch = match decision {
            EpochDecision::Keep => Vec::new(),
            EpochDecision::Patch(files) => files,
        };
        patch.extend(stale_generated);
        patch.sort_unstable_by_key(|file| file.index());
        patch.dedup();
        if patch.is_empty() {
            return;
        }
        self.patch_design_graph(db, &patch);
        let mut inner = self.inner.lock();
        inner.structure.resolution = Arc::new(ProductCell::default());
    }

    fn drop_stale_generated(&self, db: &RootDb) -> Vec<FileId> {
        let files: Vec<FileId> = self.inner.lock().generated.by_file.keys().copied().collect();
        let current: FxHashMap<FileId, u64> = files
            .into_iter()
            .filter(|&file| db.files().contains(&file))
            .map(|file| (file, <dyn PreprocDb>::compilation_unit_snapshot(db, file).fingerprint))
            .collect();
        self.inner.lock().generated.retain_current(|file, fingerprint| {
            current.get(&file).is_some_and(|&got| got == fingerprint)
        })
    }

    /// Upsert or remove `files` on the live graph. If the graph has never
    /// been built, leave the cell empty so the next request folds what exists.
    pub(crate) fn patch_design_graph(&self, db: &RootDb, files: &[FileId]) {
        if files.is_empty() {
            return;
        }
        let Some(current) = self.unit_catalog_cell().peek() else {
            return;
        };
        let generated = self.generated_units(db);
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
