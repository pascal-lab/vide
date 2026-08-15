use base_db::{
    analysis_snapshot::AnalysisSnapshotId, change::Change, diagnostics_config::DiagnosticsConfig,
    salsa::Durability, source_db::SourceDb,
};
use triomphe::Arc;

use crate::{analysis::AnalysisSnapshot, db::root_db::RootDb};

pub struct AnalysisHost {
    db: RootDb,
    snapshot_id: AnalysisSnapshotId,
}

impl AnalysisHost {
    pub fn new(lru_capacity: Option<usize>) -> AnalysisHost {
        AnalysisHost { db: RootDb::new(lru_capacity), snapshot_id: AnalysisSnapshotId::default() }
    }

    pub fn make_analysis(&self) -> AnalysisSnapshot {
        let db = self.db.clone();
        AnalysisSnapshot { db, snapshot_id: self.snapshot_id }
    }

    pub fn apply_change(&mut self, change: Change) {
        let dirty_files: Vec<_> = change.changed_files.iter().map(|file| file.file_id).collect();
        self.db.apply_change(change);
        self.db.record_dirty_files(dirty_files);
        self.advance_revision();
    }

    pub fn set_diagnostics_config(&mut self, config: Arc<DiagnosticsConfig>) {
        self.db.set_diagnostics_config_with_durability(config, Durability::HIGH);
        self.advance_revision();
    }

    fn advance_revision(&mut self) {
        self.snapshot_id = self.snapshot_id.next();
    }

    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub fn raw_db(&self) -> &RootDb {
        &self.db
    }
}

impl Default for AnalysisHost {
    fn default() -> AnalysisHost {
        AnalysisHost::new(None)
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, thread};

    use base_db::source_root::SourceRoot;
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;

    fn change_with_file_text(text: &str) -> Change {
        let file_id = FileId::from_raw(0);
        let mut file_set = FileSet::default();
        file_set.insert(file_id, VfsPath::new_virtual_path("/top.sv".to_owned()));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(file_id, text));
        change
    }

    fn modify_with_file_text(text: &str) -> Change {
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(FileId::from_raw(0), text));
        change
    }

    #[test]
    fn analysis_views_follow_input_revisions_after_snapshot_drop() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text("module old;\nendmodule\n"));

        let before = host.make_analysis();
        assert_eq!(
            before.file_text(FileId::from_raw(0)).unwrap().as_ref(),
            "module old;\nendmodule\n"
        );
        drop(before);

        host.apply_change(modify_with_file_text("module new;\nendmodule\n"));
        let after = host.make_analysis();
        assert_eq!(
            after.file_text(FileId::from_raw(0)).unwrap().as_ref(),
            "module new;\nendmodule\n"
        );
    }

    #[test]
    fn concurrent_snapshot_drop_unblocks_input_invalidation() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text("module old;\nendmodule\n"));
        let before = host.make_analysis();

        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let reader = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            assert_eq!(
                before.file_text(FileId::from_raw(0)).unwrap().as_ref(),
                "module old;\nendmodule\n"
            );
            drop(before);
        });
        ready_rx.recv().unwrap();

        let (started_tx, started_rx) = mpsc::channel();
        let updater = thread::spawn(move || {
            started_tx.send(()).unwrap();
            host.apply_change(modify_with_file_text("module new;\nendmodule\n"));
            host.make_analysis().file_text(FileId::from_raw(0)).unwrap()
        });
        started_rx.recv().unwrap();
        release_tx.send(()).unwrap();

        assert_eq!(updater.join().unwrap().as_ref(), "module new;\nendmodule\n");
        reader.join().unwrap();
    }

    #[test]
    fn read_only_views_share_one_snapshot_identity() {
        let mut host = AnalysisHost::default();
        let first = host.make_analysis();
        let second = host.make_analysis();

        assert_eq!(first.snapshot_id(), second.snapshot_id());
        assert_eq!(first.snapshot_id().get(), 0);

        drop((first, second));
        host.apply_change(Change::new());
        let changed = host.make_analysis();
        assert_eq!(changed.snapshot_id().get(), 1);
    }
}
