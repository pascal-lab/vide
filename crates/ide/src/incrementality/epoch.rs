use hir_def::item_tree::{ItemTree, StructureFingerprint};
use preproc_expand::file::HirFileId;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use vfs::FileId;

use crate::db::root_db::RootDb;

/// How a file's declaration skeleton changed relative to its pre-change
/// snapshot.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum StructureChange {
    Unchanged,
    Changed,
}

/// Outcome of comparing pre-change snapshots to the post-change item trees.
///
/// [`Keep`](EpochDecision::Keep) means body-only edits: structure products
/// survive and only dirty file shards refresh. [`Drop`](EpochDecision::Drop)
/// means a declaration skeleton changed (or we cannot prove otherwise):
/// structure products and every merge that depends on them are discarded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum EpochDecision {
    Keep,
    Drop,
}

/// A pre-change snapshot of one file's declaration structure.
#[derive(Clone)]
pub(super) struct StructureSnapshot {
    fingerprint: StructureFingerprint,
    item_tree: Arc<ItemTree>,
}

impl StructureSnapshot {
    pub(super) fn capture(db: &RootDb, file_id: FileId) -> Self {
        let tree = db.item_tree(HirFileId::File(file_id));
        Self { fingerprint: tree.structure_fingerprint(), item_tree: tree }
    }

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

    /// Compare pre-change snapshots to the post-change trees.
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
