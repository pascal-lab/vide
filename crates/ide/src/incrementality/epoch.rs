use design_graph::FileFacts;
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
/// [`Keep`](EpochDecision::Keep) means body-only edits: the name graph
/// stays. [`Patch`](EpochDecision::Patch) lists files whose CU units must
/// be upserted or removed; other files stay on the graph.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum EpochDecision {
    Keep,
    Patch(Vec<FileId>),
}

/// A pre-change snapshot of one file's L0 declaration structure.
#[derive(Clone)]
pub(super) struct StructureSnapshot {
    facts: Arc<FileFacts>,
}

impl StructureSnapshot {
    pub(super) fn capture(db: &RootDb, file_id: FileId) -> Self {
        Self { facts: db.file_facts(file_id) }
    }

    /// Classify the file's current CU declarations against this snapshot.
    fn classify(&self, db: &RootDb, file_id: FileId) -> StructureChange {
        if self.facts.same_structure(db.file_facts(file_id).as_ref()) {
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
    /// An empty epoch is [`Keep`](EpochDecision::Keep). A dirty file with no
    /// snapshot is a create (or an include-root we cannot prove stable):
    /// patch that file, do not drop the rest of the graph. A missing current
    /// file is a delete.
    pub(super) fn decide(&self, db: &RootDb) -> EpochDecision {
        if self.dirty.is_empty() {
            return EpochDecision::Keep;
        }
        let current_files = db.files();
        let mut patch = Vec::new();
        for &file_id in &self.dirty {
            let needs_patch = if !current_files.contains(&file_id) {
                true
            } else {
                match self.snapshots.get(&file_id) {
                    None => true,
                    Some(snapshot) => snapshot.classify(db, file_id) == StructureChange::Changed,
                }
            };
            if needs_patch {
                patch.push(file_id);
            }
        }
        if patch.is_empty() { EpochDecision::Keep } else { EpochDecision::Patch(patch) }
    }
}
