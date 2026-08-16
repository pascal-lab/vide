use std::{fmt, ops::Deref};

use base_db::{
    diagnostics_config::DiagnosticsConfig,
    project::ProjectConfig,
    salsa::{self, Durability},
    source_db::{FileLoader, SourceDb, SourceRootDb},
    source_root::SourceRootId,
};
use hir_def::{db::HirDefDb, def_id::DefId};
use hir_ty::db::TyDb;
use preproc_expand::{db::PreprocDb, file::HirFileId};
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::{AnchoredPath, FileId};

use crate::{
    db::{
        caches::RevisionProducts, line_index_db::LineIndexDb,
        workspace_symbol_index_db::WorkspaceSymbolIndexDb,
    },
    semantic_index::{FileModuleEdges, FileSemanticIndex, ModuleEdgeIndex, ReferenceIndex},
};

#[salsa::db]
#[derive(Clone)]
pub struct RootDb {
    storage: salsa::Storage<Self>,
    revision_products: Arc<RevisionProducts>,
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
            revision_products: Arc::new(RevisionProducts::default()),
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

    pub(crate) fn preproc_affected_files(
        &self,
        changed: impl IntoIterator<Item = FileId>,
    ) -> FxHashSet<FileId> {
        let changed = changed.into_iter().collect::<FxHashSet<_>>();
        let mut affected = changed.clone();
        let config = self.project_config();
        for profile_id in std::iter::once(None).chain(config.profile_ids().into_iter().map(Some)) {
            let plan = self.compilation_plan_for_profile(profile_id);
            let path_file_ids = self.path_file_ids();
            let mut profile_affected = plan.affected_files(changed.iter().copied());
            loop {
                let mut grew = false;
                for &includer in &plan.dynamic_include_files {
                    if profile_affected.contains(&includer) {
                        continue;
                    }
                    let Some(trace) = self.preproc_trace(includer) else {
                        continue;
                    };
                    let depends_on_affected = trace.include_edges.iter().any(|edge| {
                        trace
                            .source_buffers
                            .iter()
                            .find(|buffer| buffer.buffer_id == edge.included_buffer_id)
                            .and_then(|buffer| path_file_ids.get(&buffer.path))
                            .is_some_and(|dependency| profile_affected.contains(&dependency))
                    });
                    if depends_on_affected {
                        profile_affected.insert(includer);
                        grew = true;
                    }
                }
                let closed = plan.affected_files(profile_affected.iter().copied());
                grew |= closed.len() != profile_affected.len();
                profile_affected = closed;
                if !grew {
                    break;
                }
            }
            affected.extend(profile_affected);
        }
        affected
    }

    pub(crate) fn record_dirty_files(
        &mut self,
        files: impl IntoIterator<Item = FileId>,
        invalidate_workspace: bool,
    ) {
        if invalidate_workspace {
            self.revision_products = Arc::new(RevisionProducts::default());
            return;
        }
        let files = files.into_iter().collect::<Vec<_>>();
        if files.is_empty() {
            return;
        }
        self.revision_products = Arc::new(self.revision_products.fork());
        let mut cache = self.revision_products.lock();
        cache.indexes.reference_dirty = files.iter().copied().collect();
        if cache.revision.hir_resolution_context.is_some() {
            for &file_id in &files {
                cache.revision.structure_snapshots.entry(file_id).or_insert_with(|| {
                    let tree = self.item_tree(HirFileId::File(file_id));
                    (tree.structure_fingerprint(), tree)
                });
            }
        }
        cache.revision.resolution_dirty = files.iter().copied().collect();
        cache.indexes.request_file_index_dirty = files.iter().copied().collect();
        cache.indexes.module_edge_dirty = files.iter().copied().collect();
        for file_id in &files {
            cache.indexes.source_semantic_maps.remove(file_id);
        }
    }

    pub(crate) fn semantics(&self) -> hir_semantics::semantics::Semantics<'_, RootDb> {
        hir_semantics::semantics::Semantics::new_with_context(
            self,
            self.request_hir_resolution_context(),
        )
    }

    pub(crate) fn has_materialized_semantic_inputs(&self) -> bool {
        self.revision_products.lock().revision.semantic_inputs.is_some()
    }

    pub(crate) fn has_materialized_file_index(&self, file_id: FileId) -> bool {
        self.revision_products.lock().indexes.request_file_indexes.contains_key(&file_id)
    }

    pub(crate) fn has_materialized_module_edges(&self, root: SourceRootId) -> bool {
        self.revision_products.lock().indexes.module_edge_entries.contains_key(&root)
    }

    pub(crate) fn has_materialized_reference_index(&self, root: SourceRootId) -> bool {
        self.revision_products.lock().indexes.reference_entries.contains_key(&root)
    }

    pub(crate) fn request_source_semantic_map(
        &self,
        file_id: FileId,
    ) -> Arc<preproc_expand::macro_file::SourceSemanticMap> {
        if let Some(map) =
            self.revision_products.lock().indexes.source_semantic_maps.get(&file_id).cloned()
        {
            return map;
        }
        let map = self.source_semantic_map(file_id);
        self.revision_products.lock().indexes.source_semantic_maps.insert(file_id, map.clone());
        map
    }

    /// Resolve the structural epoch immediately after inputs change. Body-only
    /// edits keep the previous resolution products; structural edits discard
    /// them before any IDE request observes the new revision.
    pub(crate) fn finalize_structure_epoch(&self) {
        let revision = salsa::plumbing::current_revision(self);
        let mut cache = self.revision_products.lock();
        if cache.revision.hir_resolution_context.is_none() {
            return;
        }
        let dirty = cache.revision.resolution_dirty.clone();
        if dirty.is_empty() {
            return;
        }
        let current_files = self.files();
        let unchanged = dirty.iter().all(|file_id| {
            current_files.contains(file_id)
                && cache.revision.structure_snapshots.get(file_id).is_some_and(
                    |(old_fingerprint, old_tree)| {
                        let new_tree = self.item_tree(HirFileId::File(*file_id));
                        *old_fingerprint == new_tree.structure_fingerprint()
                            && **old_tree == *new_tree
                    },
                )
        });
        cache.revision.structure_snapshots.clear();
        if unchanged {
            cache.revision.resolution_built_at = Some(revision);
            return;
        }
        cache.revision.hir_resolution_context = None;
        cache.revision.semantic_inputs = None;
        cache.revision.resolution_built_at = None;
        cache.indexes.request_file_indexes.clear();
        cache.indexes.request_file_index_dirty.clear();
        cache.indexes.module_edge_entries.clear();
        cache.indexes.module_edge_dirty.clear();
    }

    pub(crate) fn request_unit_index(&self) -> Arc<hir_def::unit_index::UnitIndex> {
        self.request_hir_resolution_context().unit_index()
    }

    pub(crate) fn request_module_index(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<crate::semantic_index::ModuleIndex> {
        self.semantic_snapshot_inputs().module_index(source_root_id).unwrap_or_default()
    }

    pub(crate) fn request_module_edge_index(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<ModuleEdgeIndex> {
        let context = self.semantic_snapshot_inputs();
        let revision = salsa::plumbing::current_revision(self);
        let mut cache = self.revision_products.lock();
        let dirty = cache.indexes.module_edge_dirty.clone();
        let entry = cache.indexes.module_edge_entries.entry(source_root_id).or_default();
        if entry.built_at == Some(revision) {
            return entry.index.clone();
        }

        let source_root = self.source_root(source_root_id);
        let needs_full = dirty.is_empty() || entry.file_edges.is_empty();
        if needs_full {
            entry.file_edges = source_root
                .iter()
                .map(|file_id| {
                    (
                        file_id,
                        Arc::new(FileModuleEdges::for_file_with_indexes(
                            self,
                            file_id,
                            context.module_indexes(),
                        )),
                    )
                })
                .collect();
        } else {
            for file_id in dirty {
                if source_root.iter().any(|candidate| candidate == file_id) {
                    entry.file_edges.insert(
                        file_id,
                        Arc::new(FileModuleEdges::for_file_with_indexes(
                            self,
                            file_id,
                            context.module_indexes(),
                        )),
                    );
                }
            }
        }
        entry.index =
            Arc::new(ModuleEdgeIndex::from_file_edges(entry.file_edges.values().map(Arc::as_ref)));
        entry.built_at = Some(revision);
        entry.index.clone()
    }

    pub(crate) fn semantic_snapshot_inputs(
        &self,
    ) -> Arc<crate::semantic_index::SemanticSnapshotInputs> {
        let hir = self.request_hir_resolution_context();
        let mut cache = self.revision_products.lock();
        if let Some(context) = &cache.revision.semantic_inputs {
            return context.clone();
        }
        let context = crate::semantic_index::SemanticSnapshotInputs::from_db_with_hir(self, hir);
        cache.revision.semantic_inputs = Some(context.clone());
        context
    }

    pub(crate) fn request_file_semantic_index(&self, file_id: FileId) -> Arc<FileSemanticIndex> {
        let context = self.semantic_snapshot_inputs();
        {
            let cache = self.revision_products.lock();
            if !cache.indexes.request_file_index_dirty.contains(&file_id)
                && let Some(index) = cache.indexes.request_file_indexes.get(&file_id)
            {
                return index.clone();
            }
        }

        let index = Arc::new(FileSemanticIndex::for_file_with_context(self, file_id, &context));
        let mut cache = self.revision_products.lock();
        cache.indexes.request_file_indexes.insert(file_id, index.clone());
        cache.indexes.request_file_index_dirty.remove(&file_id);
        index
    }

    fn request_hir_resolution_context(&self) -> Arc<hir_def::pathres::ResolutionContext> {
        let revision = salsa::plumbing::current_revision(self);
        let mut cache = self.revision_products.lock();
        if cache.revision.resolution_built_at == Some(revision) {
            return cache.revision.hir_resolution_context.as_ref().unwrap().clone();
        }

        let dirty = cache.revision.resolution_dirty.clone();
        let current_files = self.files();
        let needs_rebuild = cache.revision.hir_resolution_context.is_none()
            || dirty.is_empty()
            || dirty.iter().any(|file_id| {
                !current_files.contains(file_id)
                    || cache.revision.structure_snapshots.get(file_id).is_none_or(
                        |(old_fingerprint, old_tree)| {
                            let new_tree = self.item_tree(HirFileId::File(*file_id));
                            *old_fingerprint != new_tree.structure_fingerprint()
                                || **old_tree != *new_tree
                        },
                    )
            });

        if needs_rebuild {
            let context = hir_def::pathres::ResolutionContext::from_db(self);
            cache.revision.structure_snapshots.clear();
            cache.revision.hir_resolution_context = Some(context);
            cache.revision.semantic_inputs = None;
            cache.indexes.request_file_indexes.clear();
            cache.indexes.request_file_index_dirty.clear();
            cache.indexes.module_edge_entries.clear();
            cache.indexes.module_edge_dirty.clear();
        } else {
            for file_id in dirty {
                cache.revision.structure_snapshots.remove(&file_id);
            }
        }
        cache.revision.resolution_built_at = Some(revision);
        cache.revision.hir_resolution_context.as_ref().unwrap().clone()
    }

    pub(crate) fn reference_index_for_root(
        &self,
        source_root_id: SourceRootId,
    ) -> Arc<ReferenceIndex> {
        let mut cache = self.revision_products.lock();
        let revision = salsa::plumbing::current_revision(self);
        let dirty = cache.indexes.reference_dirty.clone();
        let entry = cache.indexes.reference_entries.entry(source_root_id).or_default();
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
                    || entry
                        .item_trees
                        .get(file_id)
                        .map_or(true, |old| *old != self.item_tree(HirFileId::File(*file_id)))
            });
        if needs_full {
            let context = crate::semantic_index::SemanticSnapshotInputs::from_db(self);
            let mut file_indexes = FxHashMap::default();
            let mut item_trees = FxHashMap::default();
            for file_id in self.source_root(source_root_id).iter() {
                file_indexes.insert(
                    file_id,
                    Arc::new(crate::semantic_index::FileSemanticIndex::for_file_with_context(
                        self, file_id, &context,
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
        for file_id in &dirty {
            let old_file_index = entry.file_indexes.get(file_id).cloned().unwrap_or_default();
            let new_file_index =
                Arc::new(crate::semantic_index::FileSemanticIndex::for_file_with_context(
                    self,
                    *file_id,
                    entry.context.as_ref().unwrap(),
                ));
            Arc::make_mut(&mut entry.index).patch_file(
                self,
                *file_id,
                &old_file_index,
                &new_file_index,
            );
            entry.file_indexes.insert(*file_id, new_file_index);
            entry.item_trees.insert(*file_id, self.item_tree(HirFileId::File(*file_id)));
        }
        entry.built_at = Some(revision);
        entry.index.clone()
    }

    pub(crate) fn recursive_rename_closure(
        &self,
        def: DefId,
        visibility: crate::ScopeVisibility,
        single_file: Option<FileId>,
    ) -> Arc<Vec<DefId>> {
        Arc::new(crate::rename::recursive_rename_closure_impl(self, def, visibility, single_file))
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
