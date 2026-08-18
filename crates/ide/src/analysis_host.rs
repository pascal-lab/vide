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

use crate::{
    analysis::{AnalysisContext, AnalysisSnapshot},
    db::root_db::RootDb,
    incrementality::ProductStore,
};

pub struct AnalysisHost {
    db: RootDb,
    store: Arc<ProductStore>,
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
            store: Arc::new(ProductStore::default()),
            snapshot_id: AnalysisSnapshotId::default(),
            prewarm: None,
        }
    }

    pub fn make_analysis(&self) -> AnalysisSnapshot {
        self.signal_foreground_request();
        let db = self.db.clone();
        let salsa_revision = base_db::salsa::plumbing::current_revision(&db);
        AnalysisSnapshot {
            db,
            store: self.store.clone(),
            snapshot_id: self.snapshot_id,
            salsa_revision,
        }
    }

    pub fn apply_change(&mut self, change: Change) {
        self.cancel_prewarm();
        let dirty_files: Vec<_> = change.changed_files.iter().map(|file| file.file_id).collect();
        // Source-root changes carry file creation/deletion and path remapping.
        // Some VFS producers use `ChangedFile::create` for a full-text update
        // of an already registered file, so the per-file change kind alone is
        // not a reliable workspace-structure signal.
        // A project-config-only change (workspace switch) has no dirty files
        // and must start a new store. File create/delete also set roots, but
        // that is a graph upsert, not a workspace reset.
        let reset_products = change.project_config.is_some() && dirty_files.is_empty();
        let dependent_files =
            if reset_products { Vec::new() } else { self.store.parsed_dependents(&dirty_files) };
        let mut affected_files = dirty_files.clone();
        affected_files.extend(dependent_files.iter().copied());
        affected_files.sort_unstable_by_key(|file_id| file_id.index());
        affected_files.dedup();
        if reset_products {
            self.store = Arc::new(ProductStore::default());
            self.db.apply_change(change);
            self.start_prewarm(self.db.files().iter().copied().collect());
        } else if !affected_files.is_empty() {
            let store = self.store.fork();
            store.capture_epoch(&self.db, &dirty_files);
            // An included file can change any emitted declaration in a root.
            // There is no root-local L0 snapshot that can prove otherwise, so
            // roots named by actual include edges force a structure epoch.
            store.mark_epoch_dirty(&dependent_files);
            self.db.apply_change(change);
            store.invalidate(&self.db, &affected_files);
            self.store = Arc::new(store);
        } else {
            self.db.apply_change(change);
        }
        self.advance_revision();
        if !reset_products && !affected_files.is_empty() {
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
        let store = self.store.clone();
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
                let ctx = AnalysisContext { db: &db, store: &store };
                let hot = store.hot();
                if hot.design_graph {
                    let _ = ctx.prewarm_design_graph(&worker_cancel);
                }
                if hot.snapshot_inputs {
                    let _ = ctx.prewarm_semantic_snapshot_inputs(&worker_cancel);
                }
                let mut edge_roots = rustc_hash::FxHashSet::default();
                let mut reference_roots = rustc_hash::FxHashSet::default();
                for file_id in affected_files {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if ctx.files().contains(&file_id) {
                        let root = ctx.source_root_id(file_id);
                        if hot.module_edge_roots.contains(&root) {
                            edge_roots.insert(root);
                        }
                        if hot.name_index_roots.contains(&root) {
                            reference_roots.insert(root);
                        }
                        if hot.files.contains(&file_id) {
                            let _ = ctx.file_name_index(file_id);
                        }
                    }
                }
                for root in edge_roots {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    let _ = ctx.module_edges(root);
                }
                for root in reference_roots {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    let _ = ctx.name_index(root);
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
        // Do not join: the worker checks cancel between files and drops its
        // salsa snapshot. Joining waited out an in-flight fold on the main
        // loop and showed up as after-edit request latency.
    }

    fn join_prewarm(&mut self) {
        let Some(task) = self.prewarm.take() else {
            return;
        };
        task.cancel.store(true, Ordering::Release);
        let _ = task.worker.join();
    }

    fn signal_foreground_request(&self) {
        if let Some(task) = &self.prewarm {
            task.cancel.store(true, Ordering::Release);
        }
    }

    pub fn snapshot_id(&self) -> AnalysisSnapshotId {
        self.snapshot_id
    }

    pub fn raw_db(&self) -> &RootDb {
        self.signal_foreground_request();
        &self.db
    }

    #[cfg(test)]
    pub(crate) fn ctx(&self) -> AnalysisContext<'_> {
        AnalysisContext::new(&self.db, &self.store)
    }
}

impl Drop for AnalysisHost {
    fn drop(&mut self) {
        self.join_prewarm();
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

    fn add_second_file(text: &str) -> Change {
        let first = FileId::from_raw(0);
        let second = FileId::from_raw(1);
        let mut file_set = FileSet::default();
        file_set.insert(first, VfsPath::new_virtual_path("/top.sv".to_owned()));
        file_set.insert(second, VfsPath::new_virtual_path("/other.sv".to_owned()));
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(second, text));
        change
    }

    #[test]
    fn adding_a_file_upserts_the_existing_design_graph() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text("module first;\nendmodule\n"));
        let first = host.ctx().design_graph();
        assert_eq!(first.node_count(), 1);
        assert!(first.module_names().iter().any(|name| name == "first"));

        host.apply_change(add_second_file("module second;\nendmodule\n"));
        let both = host.ctx().design_graph();
        assert_eq!(both.node_count(), 2);
        assert!(both.module_names().iter().any(|name| name == "first"));
        assert!(both.module_names().iter().any(|name| name == "second"));
    }

    #[test]
    fn body_only_edit_keeps_the_design_graph_nodes() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text("module first;\nendmodule\n"));
        let before = host.ctx().design_graph();
        assert_eq!(before.node_count(), 1);

        host.apply_change(modify_with_file_text("module first;\n  wire x;\nendmodule\n"));
        let after = host.ctx().design_graph();
        assert_eq!(after.node_count(), 1);
        assert!(after.module_names().iter().any(|name| name == "first"));
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
