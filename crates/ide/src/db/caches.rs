use std::sync::atomic::{AtomicBool, Ordering};

use base_db::{salsa, source_root::SourceRootId};
use hir_def::{
    item_tree::{ItemTree, StructureFingerprint},
    pathres::ResolutionContext,
};
use parking_lot::{Condvar, Mutex};
use preproc_expand::macro_file::SourceSemanticMap;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use crate::semantic_index::{
    FileModuleEdges, FileSemanticIndex, ModuleEdgeIndex, ReferenceIndex, SemanticSnapshotInputs,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ProductPriority {
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
pub(super) struct ProductCell<T> {
    state: Mutex<ProductState<T>>,
    ready: Condvar,
}

impl<T> Default for ProductCell<T> {
    fn default() -> Self {
        Self { state: Mutex::new(ProductState::default()), ready: Condvar::new() }
    }
}

impl<T> ProductCell<T> {
    pub fn is_ready(&self) -> bool {
        self.state.lock().value.is_some()
    }

    pub fn get_or_compute(
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
pub(super) struct WorkspaceIndexSnapshot {
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
pub(super) struct IdeRevisionCache {
    pub hir_resolution_context: Arc<ProductCell<ResolutionContext>>,
    pub semantic_inputs: Arc<ProductCell<SemanticSnapshotInputs>>,
    pub structure_snapshots: FxHashMap<FileId, (StructureFingerprint, Arc<ItemTree>, bool)>,
    pub resolution_dirty: FxHashSet<FileId>,
    pub resolution_built_at: Option<salsa::Revision>,
}

#[derive(Clone, Default)]
pub(super) struct IdeCaches {
    pub indexes: WorkspaceIndexSnapshot,
    pub revision: IdeRevisionCache,
}

/// All lazily materialized products for exactly one input revision.
///
/// A new revision clones the shard maps (whose values are `Arc`s) and mutates
/// only affected entries. Existing `AnalysisSnapshot`s keep the previous
/// `Arc<RevisionProducts>` and can never observe products from a later edit.
#[derive(Default)]
pub(super) struct RevisionProducts {
    caches: Mutex<IdeCaches>,
}

impl std::panic::RefUnwindSafe for RevisionProducts {}
impl std::panic::UnwindSafe for RevisionProducts {}

impl RevisionProducts {
    pub fn fork(&self) -> Self {
        Self { caches: Mutex::new(self.caches.lock().clone()) }
    }

    pub fn lock(&self) -> parking_lot::MutexGuard<'_, IdeCaches> {
        self.caches.lock()
    }
}

#[derive(Clone, Default)]
pub(super) struct ReferenceIndexEntry {
    pub index: Arc<ReferenceIndex>,
    pub file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    pub item_trees: FxHashMap<FileId, Arc<ItemTree>>,
    pub context: Option<Arc<SemanticSnapshotInputs>>,
    pub built_at: Option<salsa::Revision>,
}

#[derive(Clone, Default)]
pub(super) struct ModuleEdgeEntry {
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
