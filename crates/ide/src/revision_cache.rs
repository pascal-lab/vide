use std::sync::atomic::{AtomicBool, Ordering};

use base_db::{salsa, source_root::SourceRootId};
use hir_def::{
    item_tree::{ItemTree, StructureFingerprint},
    pathres::ResolutionContext,
};
use parking_lot::{Condvar, Mutex};
use preproc_expand::{file::HirFileId, macro_file::SourceSemanticMap};
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    semantic_index::{
        FileModuleEdges, FileSemanticIndex, ModuleEdgeIndex, ReferenceIndex, SemanticSnapshotInputs,
    },
};

/// Who is asking for a product.
///
/// A [`Foreground`](ComputationPriority::Foreground) request must not wait for
/// a slower [`Background`](ComputationPriority::Background) prewarm, so it
/// supersedes an in-flight background computation. Two foreground callers
/// share one computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ComputationPriority {
    Background,
    Foreground,
}

/// One in-flight computation, tagged with the generation that started it so a
/// superseded computation can discard its result instead of publishing.
struct InFlight {
    generation: u64,
    priority: ComputationPriority,
    cancel: std::sync::Arc<AtomicBool>,
}

struct ProductState<T> {
    generation: u64,
    value: Option<Arc<T>>,
    in_flight: Option<InFlight>,
}

impl<T> Default for ProductState<T> {
    fn default() -> Self {
        Self { generation: 0, value: None, in_flight: None }
    }
}

/// A memoized revision product computed once and reused across concurrent
/// requests.
///
/// Generation model: every computation bumps a generation counter. The result
/// of a computation is published only while its generation is still current;
/// a foreground request that supersedes a background prewarm starts a newer
/// generation, and the background's late result is discarded. The mutex guards
/// state transitions only; `compute` always runs outside it.
pub(crate) struct ProductCell<T> {
    state: Mutex<ProductState<T>>,
    ready: Condvar,
}

impl<T> Default for ProductCell<T> {
    fn default() -> Self {
        Self { state: Mutex::new(ProductState::default()), ready: Condvar::new() }
    }
}

impl<T> ProductCell<T> {
    pub(crate) fn is_ready(&self) -> bool {
        self.state.lock().value.is_some()
    }

    pub(crate) fn get_or_compute(
        &self,
        priority: ComputationPriority,
        external_cancel: &AtomicBool,
        compute: impl FnOnce(&AtomicBool) -> Arc<T>,
    ) -> Option<Arc<T>> {
        let mut compute = Some(compute);
        loop {
            let (generation, cancel) = {
                let mut state = self.state.lock();
                if let Some(value) = &state.value {
                    return Some(value.clone());
                }
                if external_cancel.load(Ordering::Acquire) {
                    return None;
                }
                match &state.in_flight {
                    None => {}
                    Some(current) if priority > current.priority => {
                        current.cancel.store(true, Ordering::Release);
                    }
                    Some(_) => {
                        self.ready.wait_for(&mut state, std::time::Duration::from_millis(2));
                        continue;
                    }
                }
                state.generation += 1;
                let generation = state.generation;
                let cancel = std::sync::Arc::new(AtomicBool::new(false));
                state.in_flight = Some(InFlight { generation, priority, cancel: cancel.clone() });
                (generation, cancel)
            };

            let value = compute.take().expect("a product caller computes at most once")(&cancel);
            let mut state = self.state.lock();
            let owns_slot =
                state.in_flight.as_ref().is_some_and(|current| current.generation == generation);
            if owns_slot {
                state.in_flight = None;
                if !cancel.load(Ordering::Acquire) && !external_cancel.load(Ordering::Acquire) {
                    state.value = Some(value.clone());
                }
                self.ready.notify_all();
                return (!external_cancel.load(Ordering::Acquire)).then_some(value);
            }
            // A foreground request superseded this computation; its result is
            // intentionally discarded.
            self.ready.notify_all();
            if external_cancel.load(Ordering::Acquire) {
                return None;
            }
            return None;
        }
    }
}

/// Materialized, independently replaceable workspace index shards.
#[derive(Clone, Default)]
pub(crate) struct WorkspaceIndexSnapshot {
    pub reference_entries: FxHashMap<SourceRootId, ReferenceIndexEntry>,
    pub reference_dirty: FxHashSet<FileId>,
    pub request_file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    pub request_file_index_dirty: FxHashSet<FileId>,
    pub module_edge_entries: FxHashMap<SourceRootId, ModuleEdgeEntry>,
    pub module_edge_dirty: FxHashSet<FileId>,
    pub source_semantic_maps: FxHashMap<FileId, Arc<SourceSemanticMap>>,
}

/// A pre-change snapshot of one file's declaration structure.
#[derive(Clone)]
pub(crate) struct StructureSnapshot {
    fingerprint: StructureFingerprint,
    item_tree: Arc<ItemTree>,
}

/// How a file's structure changed relative to its pre-change snapshot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum StructureChange {
    Unchanged,
    Changed,
}

impl StructureSnapshot {
    /// Classify the file's current structure against this snapshot.
    fn classify(&self, db: &RootDb, file_id: FileId) -> StructureChange {
        // A preprocessor-independent file has a standalone declaration
        // skeleton; matching it proves the structure is unchanged without
        // entering scope or body queries. The flag is authoritative (derived
        // from the preprocessor trace), not a lexical backtick scan.
        if db.source_model(file_id).preprocessor_independent
            && let Some(skeleton) = db.declaration_skeleton(HirFileId::File(file_id))
            && skeleton.matches(&self.item_tree)
        {
            return StructureChange::Unchanged;
        }
        // Authoritative path: full item-tree equality.
        let new_tree = db.item_tree(HirFileId::File(file_id));
        if self.fingerprint == new_tree.structure_fingerprint() && *self.item_tree == *new_tree {
            StructureChange::Unchanged
        } else {
            StructureChange::Changed
        }
    }
}

/// The structural epoch: pre-change snapshots plus the dirty set, used to
/// decide whether global resolution products survive an edit.
#[derive(Clone, Default)]
pub(crate) struct StructureEpoch {
    snapshots: FxHashMap<FileId, StructureSnapshot>,
    dirty: FxHashSet<FileId>,
}

impl StructureEpoch {
    pub(crate) fn is_empty(&self) -> bool {
        self.dirty.is_empty()
    }

    pub(crate) fn clear(&mut self) {
        self.snapshots.clear();
        self.dirty.clear();
    }

    /// True when every dirty file still matches its snapshot, so the
    /// materialized resolution products remain valid for the current revision.
    /// Callers must not invoke this on an empty epoch.
    pub(crate) fn reusable(&self, db: &RootDb) -> bool {
        let current_files = db.files();
        self.dirty.iter().all(|file_id| {
            current_files.contains(file_id)
                && self.snapshots.get(file_id).is_some_and(|snapshot| {
                    snapshot.classify(db, *file_id) == StructureChange::Unchanged
                })
        })
    }
}

/// Semantic values tied to one Salsa revision and its immutable snapshots.
#[derive(Clone, Default)]
pub(crate) struct IdeRevisionCache {
    pub hir_resolution_context: Arc<ProductCell<ResolutionContext>>,
    pub semantic_inputs: Arc<ProductCell<SemanticSnapshotInputs>>,
    pub structure_epoch: StructureEpoch,
    pub resolution_built_at: Option<salsa::Revision>,
}

#[derive(Clone, Default)]
pub(crate) struct IdeCaches {
    pub indexes: WorkspaceIndexSnapshot,
    pub revision: IdeRevisionCache,
}

impl IdeCaches {
    /// Discard the resolution products and their derived indexes. The next
    /// request rebuilds them from the current structure. `resolution_built_at`
    /// is left to the caller, which also records the epoch resolution.
    pub(crate) fn discard_resolution_products(&mut self) {
        self.revision.hir_resolution_context = Arc::new(ProductCell::default());
        self.revision.semantic_inputs = Arc::new(ProductCell::default());
        self.indexes.request_file_indexes.clear();
        self.indexes.request_file_index_dirty.clear();
        self.indexes.module_edge_entries.clear();
        self.indexes.module_edge_dirty.clear();
    }
}

/// Lazily materialized workspace products scoped to one input revision.
///
/// Owned by [`crate::analysis_host::AnalysisHost`]; forked on every change so
/// previously created [`crate::analysis::AnalysisSnapshot`]s keep the previous
/// value and can never observe products from a later edit.
#[derive(Default)]
pub(crate) struct RevisionCache {
    caches: Mutex<IdeCaches>,
}

impl std::panic::RefUnwindSafe for RevisionCache {}
impl std::panic::UnwindSafe for RevisionCache {}

impl std::fmt::Debug for RevisionCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevisionCache").finish()
    }
}

impl RevisionCache {
    pub(crate) fn fork(&self) -> Self {
        Self { caches: Mutex::new(self.caches.lock().clone()) }
    }

    pub(crate) fn lock(&self) -> parking_lot::MutexGuard<'_, IdeCaches> {
        self.caches.lock()
    }

    /// Record the files made dirty by a change before Salsa applies it, so the
    /// pre-change structure snapshots can be compared against the post-change
    /// trees when the structure epoch is finalized.
    pub(crate) fn record_dirty_files(&mut self, db: &RootDb, files: &[FileId]) {
        if files.is_empty() {
            return;
        }
        // Capture pre-change snapshots outside the lock: Salsa queries must not
        // run while holding the cache mutex.
        let capture_structure = self.lock().revision.hir_resolution_context.is_ready();
        let snapshots = if capture_structure {
            files
                .iter()
                .map(|&file_id| {
                    let tree = db.item_tree(HirFileId::File(file_id));
                    (
                        file_id,
                        StructureSnapshot {
                            fingerprint: tree.structure_fingerprint(),
                            item_tree: tree,
                        },
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut cache = self.lock();
        cache.indexes.reference_dirty = files.iter().copied().collect();
        cache.indexes.request_file_index_dirty = files.iter().copied().collect();
        cache.indexes.module_edge_dirty = files.iter().copied().collect();
        for (file_id, snapshot) in snapshots {
            cache.revision.structure_epoch.snapshots.entry(file_id).or_insert(snapshot);
        }
        cache.revision.structure_epoch.dirty = files.iter().copied().collect();
        for file_id in files {
            cache.indexes.source_semantic_maps.remove(file_id);
        }
    }

    /// Resolve the structural epoch immediately after inputs change. Body-only
    /// edits keep the previous resolution products; structural edits discard
    /// them before any IDE request observes the new revision.
    pub(crate) fn finalize_structure_epoch(&self, db: &RootDb) {
        let revision = salsa::plumbing::current_revision(db);
        let epoch = {
            let cache = self.lock();
            if !cache.revision.hir_resolution_context.is_ready() {
                return;
            }
            cache.revision.structure_epoch.clone()
        };
        if epoch.is_empty() {
            return;
        }
        let reusable = epoch.reusable(db);
        let mut cache = self.lock();
        cache.revision.structure_epoch.clear();
        if !reusable {
            cache.discard_resolution_products();
        }
        cache.revision.resolution_built_at = Some(revision);
    }
}

#[derive(Clone, Default)]
pub(crate) struct ReferenceIndexEntry {
    pub index: Arc<ReferenceIndex>,
    pub file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    pub item_trees: FxHashMap<FileId, Arc<ItemTree>>,
    pub context: Option<Arc<SemanticSnapshotInputs>>,
    pub built_at: Option<salsa::Revision>,
}

#[derive(Clone, Default)]
pub(crate) struct ModuleEdgeEntry {
    pub index: Arc<ModuleEdgeIndex>,
    pub file_edges: FxHashMap<FileId, Arc<FileModuleEdges>>,
    pub built_at: Option<salsa::Revision>,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc as StdArc, mpsc};

    use super::*;

    #[test]
    fn foreground_takes_over_background_product() {
        let cell = StdArc::new(ProductCell::<u32>::default());
        let (started_tx, started_rx) = mpsc::channel();
        let background_cell = cell.clone();
        let background = std::thread::spawn(move || {
            background_cell.get_or_compute(
                ComputationPriority::Background,
                &AtomicBool::new(false),
                |cancel| {
                    started_tx.send(()).unwrap();
                    while !cancel.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                    Arc::new(1)
                },
            )
        });
        started_rx.recv().unwrap();

        let foreground = cell
            .get_or_compute(ComputationPriority::Foreground, &AtomicBool::new(false), |_| {
                Arc::new(2)
            })
            .unwrap();

        assert_eq!(*foreground, 2);
        assert!(background.join().unwrap().is_none());
        assert_eq!(
            *cell
                .get_or_compute(ComputationPriority::Foreground, &AtomicBool::new(false), |_| {
                    Arc::new(3)
                },)
                .unwrap(),
            2
        );
    }
}
