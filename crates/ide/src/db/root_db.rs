use std::{fmt, ops::Deref};

use base_db::{
    diagnostics_config::DiagnosticsConfig,
    project::ProjectConfig,
    salsa::{self, Durability},
    source_db::{FileLoader, SourceDb, SourceRootDb},
    source_root::SourceRootId,
};
use hir_def::db::HirDefDb;
use hir_def::def_id::DefId;
use hir_def::item_tree::ItemTree;
use hir_ty::db::TyDb;
use parking_lot::Mutex;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::{AnchoredPath, FileId};

use crate::db::{line_index_db::LineIndexDb, workspace_symbol_index_db::WorkspaceSymbolIndexDb};
use crate::semantic_index::{FileSemanticIndex, ReferenceIndex};

/// Per-source-root reference index, rebuilt incrementally across revisions.
///
/// Salsa revalidation is O(project) for any query that aggregates all file
/// indexes, so the merged reference index is materialized here instead of
/// being a salsa query. On a revision bump only the files changed by
/// `apply_change` are re-indexed; unchanged files reuse their cached per-file
/// indexes. A structural change to a changed file (detected by comparing its
/// `ItemTree`) conservatively falls back to a full rebuild, since that may
/// affect other files' name resolution.
struct ReferenceIndexCache {
    entries: FxHashMap<SourceRootId, ReferenceIndexEntry>,
    dirty: FxHashSet<FileId>,
}

impl Default for ReferenceIndexCache {
    fn default() -> Self {
        Self { entries: FxHashMap::default(), dirty: FxHashSet::default() }
    }
}

/// Shared handle to the reference-index cache. `parking_lot` mutexes never
/// poison, so the handle is unwind-safe: accessing it after a panic cannot
/// observe a poisoned state.
#[derive(Clone)]
struct ReferenceIndexCacheHandle(Arc<Mutex<ReferenceIndexCache>>);

// `parking_lot::Mutex` has no poisoning and `ReferenceIndexCache` holds only
// owned data, so the handle carries no unwind-sensitive invariants.
impl std::panic::RefUnwindSafe for ReferenceIndexCacheHandle {}
impl std::panic::UnwindSafe for ReferenceIndexCacheHandle {}

impl Default for ReferenceIndexCacheHandle {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(ReferenceIndexCache::default())))
    }
}

impl ReferenceIndexCacheHandle {
    fn lock(&self) -> parking_lot::MutexGuard<'_, ReferenceIndexCache> {
        self.0.lock()
    }
}

#[derive(Default)]
struct ReferenceIndexEntry {
    index: Arc<ReferenceIndex>,
    file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    item_trees: FxHashMap<FileId, Arc<ItemTree>>,
    context: Option<Arc<crate::semantic_index::IndexResolutionContext>>,
    built_at: Option<salsa::Revision>,
}

#[salsa::db]
#[derive(Clone)]
pub struct RootDb {
    storage: salsa::Storage<Self>,
    reference_index_cache: ReferenceIndexCacheHandle,
}

#[salsa::db]
impl salsa::Database for RootDb {}

#[salsa::db]
impl SourceDb for RootDb {}

#[salsa::db]
impl SourceRootDb for RootDb {}

#[salsa::db]
impl PreprocDb for RootDb {}

#[salsa::db]
impl HirDefDb for RootDb {}

#[salsa::db]
impl TyDb for RootDb {}

#[salsa::db]
impl LineIndexDb for RootDb {}

#[salsa::db]
impl WorkspaceSymbolIndexDb for RootDb {}

impl fmt::Debug for RootDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootDb").finish()
    }
}

impl FileLoader for RootDb {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let source_root_id = SourceRootDb::source_root_id(self, path.anchor);
        let source_root = SourceRootDb::source_root(self, source_root_id);
        source_root.resolve_path(path)
    }
}

impl RootDb {
    pub fn new(lru_capacity: Option<usize>) -> RootDb {
        let mut db = RootDb {
            storage: salsa::Storage::default(),
            reference_index_cache: ReferenceIndexCacheHandle::default(),
        };
        db.set_files_with_durability(Default::default(), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::HIGH);
        db.update_parse_query_lru_capacity(lru_capacity);
        db
    }

    pub fn update_parse_query_lru_capacity(&mut self, lru_capacity: Option<usize>) {
        let lru_capacity = lru_capacity.unwrap_or(DEFAULT_PARSE_LRU_CAP);
        preproc_expand::db::set_parse_lru_capacity(self, lru_capacity);
        hir_def::db::set_lru_capacity(self, lru_capacity);
    }

    pub(crate) fn record_dirty_files(&mut self, files: impl IntoIterator<Item = FileId>) {
        self.reference_index_cache.lock().dirty.extend(files);
    }

    pub(crate) fn reference_index_for_root(&self, source_root_id: SourceRootId) -> Arc<ReferenceIndex> {
        let mut cache = self.reference_index_cache.lock();
        let revision = salsa::plumbing::current_revision(self);
        let dirty = std::mem::take(&mut cache.dirty);
        let entry = cache.entries.entry(source_root_id).or_default();
        if entry.built_at == Some(revision) {
            return entry.index.clone();
        }

        let current_files = self.files();

        // A structural change (or first build) forces a full rebuild, because a
        // changed definition can affect name resolution in every other file.
        let needs_full = dirty.is_empty()
            || entry.file_indexes.is_empty()
            || dirty.iter().any(|file_id| {
                !current_files.contains(file_id)
                    || entry.item_trees.get(file_id).map_or(true, |old| {
                        *old != self.item_tree(HirFileId::File(*file_id))
                    })
            });

        if needs_full {
            let context = crate::semantic_index::IndexResolutionContext::from_db(self);
            let mut file_indexes = FxHashMap::default();
            let mut item_trees = FxHashMap::default();
            for file_id in self.source_root(source_root_id).iter() {
                file_indexes.insert(
                    file_id,
                    Arc::new(crate::semantic_index::FileSemanticIndex::for_file_with_context(
                        self,
                        file_id,
                        &context,
                    )),
                );
                item_trees.insert(file_id, self.item_tree(HirFileId::File(file_id)));
            }
            let index = Arc::new(ReferenceIndex::from_file_indexes(self, &file_indexes));
            entry.index = index.clone();
            entry.file_indexes = file_indexes;
            entry.item_trees = item_trees;
            entry.context = Some(context);
            entry.built_at = Some(revision);
            return index;
        }

        // Incremental: patch the cached index with each dirty file's new
        // contribution, reusing cached name/ranges for existing definitions.
        let mut index = (*entry.index).clone();
        for file_id in &dirty {
            let old_file_index = entry.file_indexes.get(file_id).cloned().unwrap_or_default();
            let new_file_index = Arc::new(
                crate::semantic_index::FileSemanticIndex::for_file_with_context(
                    self,
                    *file_id,
                    entry.context.as_ref().unwrap(),
                ),
            );
            index =
                ReferenceIndex::patch_file(self, &index, *file_id, &old_file_index, &new_file_index);
            entry.file_indexes.insert(*file_id, new_file_index);
            entry.item_trees.insert(*file_id, self.item_tree(HirFileId::File(*file_id)));
        }
        entry.index = Arc::new(index);
        entry.built_at = Some(revision);
        entry.index.clone()
    }

    pub(crate) fn recursive_rename_closure(
        &self,
        def: DefId,
        visibility: crate::ScopeVisibility,
        single_file: Option<FileId>,
    ) -> Arc<Vec<DefId>> {
        Arc::new(crate::rename::recursive_rename_closure_impl(
            self,
            def,
            visibility,
            single_file,
        ))
    }
}

/// Default memo capacity for per-file parse/HIR queries. Salsa revalidation
/// recomputes evicted memos after a revision bump, so a capacity below the
/// project's per-file working set turns incremental rebuilds into repeated
/// re-parse/re-lower work. 1024 covers small-to-medium projects without
/// pinning an unbounded number of parse trees.
pub const DEFAULT_PARSE_LRU_CAP: usize = 1024;
impl RootDb {}

// RootDb is the concrete IDE database; expose the workspace query surface
// without maintaining a second set of forwarding methods.
impl Deref for RootDb {
    type Target = dyn WorkspaceSymbolIndexDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}
