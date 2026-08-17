use hir_def::decl_shard::FileDeclShard;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use crate::db::root_db::RootDb;

/// How a file's L0 compilation-unit declarations changed relative to its
/// pre-change snapshot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum StructureChange {
    Unchanged,
    Changed,
}

/// Outcome of comparing pre-change snapshots to the post-change L0 shards.
///
/// [`Keep`](EpochDecision::Keep) means body-only edits: structure products
/// survive and only dirty file shards refresh. [`Drop`](EpochDecision::Drop)
/// means a compilation-unit declaration or import changed (or we cannot
/// prove otherwise): structure products and every merge that depends on
/// them are discarded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum EpochDecision {
    Keep,
    Drop,
}

/// A pre-change snapshot of one file's L0 declaration structure.
#[derive(Clone)]
pub(super) struct StructureSnapshot {
    shard: Arc<FileDeclShard>,
}

impl StructureSnapshot {
    pub(super) fn capture(db: &RootDb, file_id: FileId) -> Self {
        Self { shard: db.file_decl_shard(file_id) }
    }

    /// Classify the file's current CU declarations against this snapshot.
    fn classify(&self, db: &RootDb, file_id: FileId) -> StructureChange {
        if self.shard.same_structure(db.file_decl_shard(file_id).as_ref()) {
            StructureChange::Unchanged
        } else {
            StructureChange::Changed
        }
    }
}

/// The structural epoch: pre-change snapshots plus the dirty set, used to
/// decide whether global resolution products survive an edit.
///
/// Lives only between [`super::store::ProductStore::capture_epoch`] and
/// [`super::store::ProductStore::invalidate`]. The request path never reads it.
#[derive(Clone, Default)]
pub(super) struct StructureEpoch {
    snapshots: FxHashMap<FileId, StructureSnapshot>,
    dirty: FxHashSet<FileId>,
}

impl StructureEpoch {
    pub(super) fn is_empty(&self) -> bool {
        self.dirty.is_empty()
    }

    pub(super) fn record(&mut self, files: impl IntoIterator<Item = (FileId, StructureSnapshot)>) {
        for (file_id, snapshot) in files {
            self.snapshots.entry(file_id).or_insert(snapshot);
            self.dirty.insert(file_id);
        }
    }

    pub(super) fn mark_dirty(&mut self, files: &[FileId]) {
        self.dirty.extend(files.iter().copied());
    }

    pub(super) fn clear(&mut self) {
        self.snapshots.clear();
        self.dirty.clear();
    }

    /// Compare pre-change snapshots to the post-change L0 shards.
    ///
    /// An empty epoch is [`Keep`](EpochDecision::Keep): nothing changed that
    /// we know about. Missing snapshots for a dirty file cannot prove the
    /// skeleton is unchanged, so they are [`Drop`](EpochDecision::Drop).
    pub(super) fn decide(&self, db: &RootDb) -> EpochDecision {
        if self.dirty.is_empty() {
            return EpochDecision::Keep;
        }
        let current_files = db.files();
        let reusable = self.dirty.iter().all(|file_id| {
            current_files.contains(file_id)
                && self.snapshots.get(file_id).is_some_and(|snapshot| {
                    snapshot.classify(db, *file_id) == StructureChange::Unchanged
                })
        });
        if reusable { EpochDecision::Keep } else { EpochDecision::Drop }
    }
}
