use rustc_hash::FxHashMap;
use triomphe::Arc;
use vfs::FileId;

use crate::{
    analysis::AnalysisContext,
    name_index::{FileNameIndex, NameIndex},
};

#[derive(Clone, Default)]
pub(super) struct GenArc<T> {
    pub value: Arc<T>,
    pub built_gen: u64,
}

#[derive(Clone, Default)]
pub(super) struct NameIndexEntry {
    pub index: Arc<NameIndex>,
    pub file_indexes: FxHashMap<FileId, Arc<FileNameIndex>>,
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

impl NameIndexEntry {
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
        let full =
            self.file_indexes.is_empty() || has_removed_files(&self.file_indexes, root_files);

        if full {
            self.file_indexes = root_files
                .iter()
                .map(|&file_id| (file_id, Arc::new(FileNameIndex::for_file(ctx.db, file_id))))
                .collect();
            self.shard_gens =
                root_files.iter().map(|&file_id| (file_id, file_gen(gens, file_id))).collect();
        } else {
            for file_id in stale {
                self.file_indexes
                    .insert(file_id, Arc::new(FileNameIndex::for_file(ctx.db, file_id)));
                self.shard_gens.insert(file_id, file_gen(gens, file_id));
            }
            self.file_indexes.retain(|file_id, _| root_files.contains(file_id));
            self.shard_gens.retain(|file_id, _| root_files.contains(file_id));
        }

        self.index = Arc::new(NameIndex::from_file_indexes(&self.file_indexes));
    }
}
