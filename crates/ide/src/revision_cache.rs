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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ProductPriority {
    Background,
    Foreground,
}

struct ComputingProduct {
    generation: u64,
    priority: ProductPriority,
    cancel: std::sync::Arc<AtomicBool>,
}

struct ProductState<T> {
    generation: u64,
    value: Option<Arc<T>>,
    computing: Option<ComputingProduct>,
}

impl<T> Default for ProductState<T> {
    fn default() -> Self {
        Self { generation: 0, value: None, computing: None }
    }
}

/// One revision product with foreground takeover and lock-free computation.
/// The mutex protects state transitions only; `compute` always runs outside it.
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
        priority: ProductPriority,
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
                match &state.computing {
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
                state.computing =
                    Some(ComputingProduct { generation, priority, cancel: cancel.clone() });
                (generation, cancel)
            };

            let value = compute.take().expect("a product caller computes at most once")(&cancel);
            let mut state = self.state.lock();
            let owns_slot =
                state.computing.as_ref().is_some_and(|current| current.generation == generation);
            if owns_slot {
                state.computing = None;
                if !cancel.load(Ordering::Acquire) && !external_cancel.load(Ordering::Acquire) {
                    state.value = Some(value.clone());
                }
                self.ready.notify_all();
                return (!external_cancel.load(Ordering::Acquire)).then_some(value);
            }
            // A foreground caller took over this background computation. Its
            // result is intentionally discarded; wait for the winning slot.
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

/// Semantic values tied to one Salsa revision and its immutable snapshots.
#[derive(Clone, Default)]
pub(crate) struct IdeRevisionCache {
    pub hir_resolution_context: Arc<ProductCell<ResolutionContext>>,
    pub semantic_inputs: Arc<ProductCell<SemanticSnapshotInputs>>,
    pub structure_snapshots: FxHashMap<FileId, (StructureFingerprint, Arc<ItemTree>, bool)>,
    pub resolution_dirty: FxHashSet<FileId>,
    pub resolution_built_at: Option<salsa::Revision>,
}

#[derive(Clone, Default)]
pub(crate) struct IdeCaches {
    pub indexes: WorkspaceIndexSnapshot,
    pub revision: IdeRevisionCache,
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
        let capture_structure = self.lock().revision.hir_resolution_context.is_ready();
        let structure_snapshots = if capture_structure {
            files
                .iter()
                .map(|&file_id| {
                    let tree = db.item_tree(HirFileId::File(file_id));
                    // A backtick is the lexical introducer for every
                    // preprocessor directive and macro call. Its absence is a
                    // cheap, conservative proof that the old source can use
                    // the standalone declaration skeleton; false positives
                    // (for example a backtick in a string) only take the slow
                    // authoritative path.
                    let allow_skeleton = !db.file_text(file_id).contains('`');
                    (file_id, (tree.structure_fingerprint(), tree, allow_skeleton))
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut cache = self.lock();
        cache.indexes.reference_dirty = files.iter().copied().collect();
        for (file_id, snapshot) in structure_snapshots {
            cache.revision.structure_snapshots.entry(file_id).or_insert(snapshot);
        }
        cache.revision.resolution_dirty = files.iter().copied().collect();
        cache.indexes.request_file_index_dirty = files.iter().copied().collect();
        cache.indexes.module_edge_dirty = files.iter().copied().collect();
        for file_id in files {
            cache.indexes.source_semantic_maps.remove(file_id);
        }
    }

    /// Resolve the structural epoch immediately after inputs change. Body-only
    /// edits keep the previous resolution products; structural edits discard
    /// them before any IDE request observes the new revision.
    pub(crate) fn finalize_structure_epoch(&self, db: &RootDb) {
        let revision = salsa::plumbing::current_revision(db);
        let cache = self.lock();
        if !cache.revision.hir_resolution_context.is_ready() {
            return;
        }
        let dirty = cache.revision.resolution_dirty.clone();
        if dirty.is_empty() {
            return;
        }
        let snapshots = dirty
            .iter()
            .filter_map(|file_id| {
                cache
                    .revision
                    .structure_snapshots
                    .get(file_id)
                    .cloned()
                    .map(|snapshot| (*file_id, snapshot))
            })
            .collect::<FxHashMap<_, _>>();
        drop(cache);
        let current_files = db.files();
        let unchanged = dirty.iter().all(|file_id| {
            current_files.contains(file_id)
                && snapshots.get(file_id).is_some_and(
                    |(old_fingerprint, old_tree, allow_skeleton)| {
                        structure_matches(db, *file_id, *old_fingerprint, old_tree, *allow_skeleton)
                    },
                )
        });
        let mut cache = self.lock();
        cache.revision.structure_snapshots.clear();
        if unchanged {
            cache.revision.resolution_built_at = Some(revision);
            return;
        }
        cache.revision.hir_resolution_context = Arc::new(ProductCell::default());
        cache.revision.semantic_inputs = Arc::new(ProductCell::default());
        cache.revision.resolution_built_at = None;
        cache.indexes.request_file_indexes.clear();
        cache.indexes.request_file_index_dirty.clear();
        cache.indexes.module_edge_entries.clear();
        cache.indexes.module_edge_dirty.clear();
    }
}

pub(crate) fn structure_matches(
    db: &RootDb,
    file_id: FileId,
    old_fingerprint: StructureFingerprint,
    old_tree: &ItemTree,
    allow_skeleton: bool,
) -> bool {
    if allow_skeleton
        && let Some(skeleton) = db.declaration_skeleton(HirFileId::File(file_id))
        && skeleton.preprocessor_independent()
        && skeleton.matches(old_tree)
    {
        return true;
    }
    let new_tree = db.item_tree(HirFileId::File(file_id));
    old_fingerprint == new_tree.structure_fingerprint() && *old_tree == *new_tree
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
                ProductPriority::Background,
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
            .get_or_compute(ProductPriority::Foreground, &AtomicBool::new(false), |_| Arc::new(2))
            .unwrap();

        assert_eq!(*foreground, 2);
        assert!(background.join().unwrap().is_none());
        assert_eq!(
            *cell
                .get_or_compute(ProductPriority::Foreground, &AtomicBool::new(false), |_| Arc::new(
                    3
                ),)
                .unwrap(),
            2
        );
    }
}
