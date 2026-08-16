use std::{
    sync::{
        Arc as StdArc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use base_db::{
    analysis_snapshot::AnalysisSnapshotId,
    change::Change,
    diagnostics_config::DiagnosticsConfig,
    salsa::Durability,
    source_db::{SourceDb, SourceRootDb},
};
use triomphe::Arc;

use crate::{analysis::AnalysisSnapshot, db::root_db::RootDb};

pub struct AnalysisHost {
    db: RootDb,
    snapshot_id: AnalysisSnapshotId,
    prewarm: Option<PrewarmTask>,
}

struct PrewarmTask {
    cancel: StdArc<AtomicBool>,
    worker: JoinHandle<()>,
}

impl AnalysisHost {
    pub fn new(lru_capacity: Option<usize>) -> AnalysisHost {
        AnalysisHost {
            db: RootDb::new(lru_capacity),
            snapshot_id: AnalysisSnapshotId::default(),
            prewarm: None,
        }
    }

    pub fn make_analysis(&self) -> AnalysisSnapshot {
        let db = self.db.clone();
        let salsa_revision = base_db::salsa::plumbing::current_revision(&db);
        AnalysisSnapshot { db, snapshot_id: self.snapshot_id, salsa_revision }
    }

    pub fn apply_change(&mut self, change: Change) {
        self.cancel_prewarm();
        let dirty_files: Vec<_> = change.changed_files.iter().map(|file| file.file_id).collect();
        // Source-root changes carry file creation/deletion and path remapping.
        // Some VFS producers use `ChangedFile::create` for a full-text update
        // of an already registered file, so the per-file change kind alone is
        // not a reliable workspace-structure signal.
        let invalidate_workspace = change.roots.is_some() || change.project_config.is_some();
        let affected_files = if invalidate_workspace {
            dirty_files
        } else {
            self.db.preproc_affected_files(dirty_files).into_iter().collect()
        };
        self.db.record_dirty_files(affected_files.iter().copied(), invalidate_workspace);
        self.db.apply_change(change);
        self.db.finalize_structure_epoch();
        self.advance_revision();
        if !invalidate_workspace && !affected_files.is_empty() {
            self.start_prewarm(affected_files);
        }
    }

    pub fn set_diagnostics_config(&mut self, config: Arc<DiagnosticsConfig>) {
        self.db.set_diagnostics_config_with_durability(config, Durability::HIGH);
        self.advance_revision();
    }

    fn advance_revision(&mut self) {
        self.snapshot_id = self.snapshot_id.next();
    }

    fn start_prewarm(&mut self, affected_files: Vec<vfs::FileId>) {
        let db = self.db.clone();
        let cancel = StdArc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let worker = thread::Builder::new()
            .name("vide-revision-prewarm".to_owned())
            .spawn(move || {
                // Give latency-sensitive foreground requests first access to
                // the new revision. Prewarm only starts once the edit has been
                // idle briefly, and cancellation stays responsive to typing.
                for _ in 0..10 {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    thread::sleep(std::time::Duration::from_millis(5));
                }
                if db.has_materialized_semantic_inputs() {
                    let _ = db.semantic_snapshot_inputs();
                }
                let mut roots = rustc_hash::FxHashSet::default();
                for file_id in affected_files {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if db.files().contains(&file_id) {
                        roots.insert(db.source_root_id(file_id));
                        if db.has_materialized_file_index(file_id) {
                            let _ = db.request_file_semantic_index(file_id);
                        }
                    }
                }
                for root in roots {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if db.has_materialized_module_edges(root) {
                        let _ = db.request_module_edge_index(root);
                    }
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if db.has_materialized_reference_index(root) {
                        let _ = db.reference_index_for_root(root);
                    }
                }
            })
            .expect("failed to spawn revision prewarm worker");
        self.prewarm = Some(PrewarmTask { cancel, worker });
    }

    fn cancel_prewarm(&mut self) {
        let Some(task) = self.prewarm.take() else {
            return;
        };
        task.cancel.store(true, Ordering::Release);
        let _ = task.worker.join();
    }

    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub fn raw_db(&self) -> &RootDb {
        &self.db
    }
}

impl Drop for AnalysisHost {
    fn drop(&mut self) {
        self.cancel_prewarm();
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
    use utils::paths::{AbsPathBuf, Utf8PathBuf};
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

    fn change_with_include() -> Change {
        let top = FileId::from_raw(0);
        let header = FileId::from_raw(1);
        let mut file_set = FileSet::default();
        let root = if cfg!(windows) { r"C:\repo" } else { "/repo" };
        let top_path = AbsPathBuf::assert(Utf8PathBuf::from(format!("{root}/top.sv")));
        let header_path = AbsPathBuf::assert(Utf8PathBuf::from(format!("{root}/defs.svh")));
        file_set.insert(top, VfsPath::from(top_path));
        file_set.insert(header, VfsPath::from(header_path));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local_with_source_files(file_set, vec![top])]);
        change.add_changed_file(ChangedFile::create(
            top,
            "`include \"defs.svh\"\nmodule top; endmodule\n",
        ));
        change.add_changed_file(ChangedFile::create(header, "`define VALUE 1\n"));
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

    #[test]
    fn include_changes_mark_includers_affected() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_include());

        let affected = host.db.preproc_affected_files([FileId::from_raw(1)]);

        assert!(affected.contains(&FileId::from_raw(0)));
    }
}
