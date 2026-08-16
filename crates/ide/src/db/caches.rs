use base_db::{salsa, source_root::SourceRootId};
use hir_def::{item_tree::ItemTree, pathres::ResolutionContext};
use parking_lot::Mutex;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use utils::line_index::TextRange;
use vfs::FileId;

use crate::semantic_index::{
    FileModuleEdges, FileSemanticIndex, ModuleEdgeIndex, ReferenceIndex, SemanticSnapshotInputs,
};

/// Materialized, independently replaceable workspace index shards.
#[derive(Default)]
pub(super) struct WorkspaceIndexStore {
    pub reference_entries: FxHashMap<SourceRootId, ReferenceIndexEntry>,
    pub reference_dirty: FxHashSet<FileId>,
    pub request_file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    pub request_file_index_dirty: FxHashSet<FileId>,
    pub module_edge_entries: FxHashMap<SourceRootId, ModuleEdgeEntry>,
    pub module_edge_dirty: FxHashSet<FileId>,
}

/// Semantic values tied to one Salsa revision and its immutable snapshots.
#[derive(Default)]
pub(super) struct IdeRevisionCache {
    pub hir_resolution_context: Option<Arc<ResolutionContext>>,
    pub semantic_inputs: Option<Arc<SemanticSnapshotInputs>>,
    pub resolution_item_trees: FxHashMap<FileId, Arc<ItemTree>>,
    pub resolution_dirty: FxHashSet<FileId>,
    pub resolution_built_at: Option<salsa::Revision>,
    pub macro_generated_origins: FxHashMap<(FileId, TextRange), bool>,
}

#[derive(Default)]
pub(super) struct IdeCaches {
    pub indexes: WorkspaceIndexStore,
    pub revision: IdeRevisionCache,
}

/// Snapshots cloned from one `RootDb` share the same cache generation. Salsa
/// serializes input mutation against live snapshots, so a generation cannot be
/// mutated while a request observes it.
#[derive(Clone, Default)]
pub(super) struct IdeCachesHandle(Arc<Mutex<IdeCaches>>);

impl std::panic::RefUnwindSafe for IdeCachesHandle {}
impl std::panic::UnwindSafe for IdeCachesHandle {}

impl IdeCachesHandle {
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, IdeCaches> {
        self.0.lock()
    }
}

#[derive(Default)]
pub(super) struct ReferenceIndexEntry {
    pub index: Arc<ReferenceIndex>,
    pub file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    pub item_trees: FxHashMap<FileId, Arc<ItemTree>>,
    pub context: Option<Arc<SemanticSnapshotInputs>>,
    pub built_at: Option<salsa::Revision>,
}

#[derive(Default)]
pub(super) struct ModuleEdgeEntry {
    pub index: Arc<ModuleEdgeIndex>,
    pub file_edges: FxHashMap<FileId, Arc<FileModuleEdges>>,
    pub built_at: Option<salsa::Revision>,
}
