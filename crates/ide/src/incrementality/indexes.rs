use hir_def::item_tree::ItemTree;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use triomphe::Arc;
use vfs::FileId;

use crate::{
    analysis::AnalysisContext,
    db::root_db::RootDb,
    semantic_index::{
        FileModuleEdges, FileSemanticIndex, ModuleEdgeIndex, ReferenceIndex, SemanticSnapshotInputs,
    },
};

/// How a merged index should refresh against the current generation clock.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Rebuild {
    /// Body-only: replace each stale file's contribution in place.
    Patch,
    /// First build, a file disappeared, or a dirty file's item tree changed:
    /// nameres is global, so the whole merge is rebuilt.
    Full,
}

#[derive(Clone, Default)]
pub(super) struct GenArc<T> {
    pub value: Arc<T>,
    pub built_gen: u64,
}

#[derive(Clone, Default)]
pub(super) struct ReferenceIndexEntry {
    pub index: Arc<ReferenceIndex>,
    pub file_indexes: FxHashMap<FileId, Arc<FileSemanticIndex>>,
    pub item_trees: FxHashMap<FileId, Arc<ItemTree>>,
    pub context: Option<Arc<SemanticSnapshotInputs>>,
    pub shard_gens: FxHashMap<FileId, u64>,
}

#[derive(Clone, Default)]
pub(super) struct ModuleEdgeEntry {
    pub index: Arc<ModuleEdgeIndex>,
    pub file_edges: FxHashMap<FileId, Arc<FileModuleEdges>>,
    pub shard_gens: FxHashMap<FileId, u64>,
}

pub(super) fn file_gen(gens: &FxHashMap<FileId, u64>, file_id: FileId) -> u64 {
    gens.get(&file_id).copied().unwrap_or(0)
}

pub(super) fn stale_files(
    root_files: &[FileId],
    shard_gens: &FxHashMap<FileId, u64>,
    gens: &FxHashMap<FileId, u64>,
) -> Vec<FileId> {
    root_files
        .iter()
        .copied()
        .filter(|file_id| shard_gens.get(file_id).copied() != Some(file_gen(gens, *file_id)))
        .collect()
}

fn has_removed_files(existing: &FxHashMap<FileId, impl Sized>, root_files: &[FileId]) -> bool {
    existing.len() != root_files.len()
        || existing.keys().any(|file_id| !root_files.contains(file_id))
}

impl ReferenceIndexEntry {
    pub(super) fn is_fresh(&self, root_files: &[FileId], gens: &FxHashMap<FileId, u64>) -> bool {
        !self.file_indexes.is_empty()
            && !has_removed_files(&self.file_indexes, root_files)
            && stale_files(root_files, &self.shard_gens, gens).is_empty()
    }

    pub(super) fn refresh(
        &mut self,
        ctx: &AnalysisContext<'_>,
        root_files: &[FileId],
        gens: &FxHashMap<FileId, u64>,
    ) {
        let stale = stale_files(root_files, &self.shard_gens, gens);
        let policy = if self.file_indexes.is_empty()
            || has_removed_files(&self.file_indexes, root_files)
            || stale.iter().any(|file_id| structure_changed(ctx.db, &self.item_trees, *file_id))
        {
            Rebuild::Full
        } else {
            Rebuild::Patch
        };

        match policy {
            Rebuild::Full => {
                let context = ctx.semantic_snapshot_inputs();
                let mut file_indexes = FxHashMap::default();
                let mut item_trees = FxHashMap::default();
                let mut shard_gens = FxHashMap::default();
                for &file_id in root_files {
                    file_indexes.insert(
                        file_id,
                        Arc::new(FileSemanticIndex::for_file_with_context(
                            ctx.db, file_id, &context,
                        )),
                    );
                    item_trees.insert(file_id, ctx.db.item_tree(HirFileId::File(file_id)));
                    shard_gens.insert(file_id, file_gen(gens, file_id));
                }
                self.index = Arc::new(ReferenceIndex::from_file_indexes(ctx.db, &file_indexes));
                self.file_indexes = file_indexes;
                self.item_trees = item_trees;
                self.shard_gens = shard_gens;
                self.context = Some(context);
            }
            Rebuild::Patch => {
                for file_id in stale {
                    let old_file_index =
                        self.file_indexes.get(&file_id).cloned().unwrap_or_default();
                    let new_file_index = Arc::new(FileSemanticIndex::for_file_with_context(
                        ctx.db,
                        file_id,
                        self.context.as_ref().expect("patch requires a prior full build"),
                    ));
                    Arc::make_mut(&mut self.index).patch_file(
                        ctx.db,
                        file_id,
                        &old_file_index,
                        &new_file_index,
                    );
                    self.file_indexes.insert(file_id, new_file_index);
                    self.item_trees.insert(file_id, ctx.db.item_tree(HirFileId::File(file_id)));
                    self.shard_gens.insert(file_id, file_gen(gens, file_id));
                }
            }
        }
    }
}

impl ModuleEdgeEntry {
    pub(super) fn is_fresh(&self, root_files: &[FileId], gens: &FxHashMap<FileId, u64>) -> bool {
        !self.file_edges.is_empty()
            && !has_removed_files(&self.file_edges, root_files)
            && stale_files(root_files, &self.shard_gens, gens).is_empty()
    }

    pub(super) fn refresh(
        &mut self,
        ctx: &AnalysisContext<'_>,
        root_files: &[FileId],
        gens: &FxHashMap<FileId, u64>,
    ) {
        let stale = stale_files(root_files, &self.shard_gens, gens);
        let context = ctx.semantic_snapshot_inputs();
        let full = self.file_edges.is_empty() || has_removed_files(&self.file_edges, root_files);

        if full {
            self.file_edges = root_files
                .iter()
                .map(|&file_id| {
                    (
                        file_id,
                        Arc::new(FileModuleEdges::for_file_with_indexes(
                            ctx.db,
                            file_id,
                            context.module_indexes(),
                        )),
                    )
                })
                .collect();
            self.shard_gens =
                root_files.iter().map(|&file_id| (file_id, file_gen(gens, file_id))).collect();
        } else {
            for file_id in stale {
                self.file_edges.insert(
                    file_id,
                    Arc::new(FileModuleEdges::for_file_with_indexes(
                        ctx.db,
                        file_id,
                        context.module_indexes(),
                    )),
                );
                self.shard_gens.insert(file_id, file_gen(gens, file_id));
            }
            self.file_edges.retain(|file_id, _| root_files.contains(file_id));
            self.shard_gens.retain(|file_id, _| root_files.contains(file_id));
        }

        self.index =
            Arc::new(ModuleEdgeIndex::from_file_edges(self.file_edges.values().map(Arc::as_ref)));
    }
}

fn structure_changed(
    db: &RootDb,
    item_trees: &FxHashMap<FileId, Arc<ItemTree>>,
    file_id: FileId,
) -> bool {
    item_trees.get(&file_id).is_none_or(|old| *old != db.item_tree(HirFileId::File(file_id)))
}
