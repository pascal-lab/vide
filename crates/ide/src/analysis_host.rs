use std::{
    sync::{
        Arc as StdArc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use base_db::{
    analysis_snapshot::AnalysisSnapshotId, change::Change, diagnostics_config::DiagnosticsConfig,
    salsa::Durability, source_db::SourceDb,
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
        // File create/delete also set roots; that is a graph upsert, not a
        // workspace reset. A new project config changes profile predefines
        // for every file, not just the dirty set — incremental epoch compare
        // of dirty files would Keep a graph whose other files still have the
        // old facts.
        let reset_products = change.project_config.is_some();
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
                for file_id in affected_files {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if db.file_kind(file_id).is_semantic_compilation_unit() {
                        let _ = <dyn design_graph::DesignGraphDb>::file_facts(&db, file_id);
                    }
                }
                let _ = ctx.prewarm_design_graph(&worker_cancel);
                if !worker_cancel.load(Ordering::Acquire) {
                    let _ = ctx.prewarm_resolution(&worker_cancel);
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

    fn two_file_workspace(first: &str, second: &str) -> Change {
        let first_id = FileId::from_raw(0);
        let second_id = FileId::from_raw(1);
        let mut file_set = FileSet::default();
        file_set.insert(first_id, VfsPath::new_virtual_path("/gen.sv".to_owned()));
        file_set.insert(second_id, VfsPath::new_virtual_path("/other.sv".to_owned()));
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.add_changed_file(ChangedFile::create(first_id, first));
        change.add_changed_file(ChangedFile::create(second_id, second));
        change
    }

    fn modify_file(file_id: FileId, text: &str) -> Change {
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(file_id, text));
        change
    }

    fn project_config_with_predefines(predefines: Vec<String>) -> Change {
        use base_db::project::{
            CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig,
        };
        use base_db::source_root::SourceRootId;
        use triomphe::Arc;
        let mut change = Change::new();
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig::with_predefine_strings(predefines, Vec::new()),
            }],
        )));
        change
    }

    fn goto_names(host: &AnalysisHost, file_id: FileId, text: &str, needle: &str) -> Vec<String> {
        let offset = utils::line_index::TextSize::from(text.find(needle).expect(needle) as u32);
        host.make_analysis()
            .goto_definition(crate::FilePosition { file_id, offset })
            .unwrap()
            .map(|hit| {
                hit.info
                    .into_iter()
                    .filter_map(|nav| nav.name.map(|name| name.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn generated_unit_rename_invalidates_overlay() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text(
            "`define GEN(name) module name; endmodule\n`GEN(foo)\nmodule top;\nendmodule\n",
        ));
        let _ = host.ctx().parse_file(FileId::from_raw(0));
        let before = host.ctx().design_graph();
        assert!(
            before.module_names().iter().any(|name| name == "foo"),
            "{:?}",
            before.module_names()
        );
        assert!(
            before.module_names().iter().any(|name| name == "top"),
            "{:?}",
            before.module_names()
        );

        host.apply_change(modify_with_file_text(
            "`define GEN(name) module name; endmodule\n`GEN(bar)\nmodule top;\nendmodule\n",
        ));
        let after_edit = host.ctx().design_graph();
        assert!(
            !after_edit.module_names().iter().any(|name| name == "foo"),
            "stale generated name foo must not survive the edit: {:?}",
            after_edit.module_names()
        );
        assert!(
            after_edit.module_names().iter().any(|name| name == "top"),
            "{:?}",
            after_edit.module_names()
        );

        let _ = host.ctx().parse_file(FileId::from_raw(0));
        let after_reparse = host.ctx().design_graph();
        assert!(
            !after_reparse.module_names().iter().any(|name| name == "foo"),
            "{:?}",
            after_reparse.module_names()
        );
        assert!(
            after_reparse.module_names().iter().any(|name| name == "bar"),
            "{:?}",
            after_reparse.module_names()
        );
    }

    #[test]
    fn generated_unit_rename_invalidates_cross_file_goto() {
        let gen_foo =
            "`define GEN(name) module name; endmodule\n`GEN(foo)\nmodule top;\nendmodule\n";
        let gen_bar =
            "`define GEN(name) module name; endmodule\n`GEN(bar)\nmodule top;\nendmodule\n";
        let other = "module other;\n  foo u_foo();\n  bar u_bar();\nendmodule\n";
        let generator = FileId::from_raw(0);
        let user = FileId::from_raw(1);

        let mut host = AnalysisHost::default();
        host.apply_change(two_file_workspace(gen_foo, other));
        let _ = host.ctx().parse_file(generator);
        assert_eq!(goto_names(&host, user, other, "foo u_foo"), ["foo"]);
        assert!(goto_names(&host, user, other, "bar u_bar").is_empty(), "bar is not generated yet");

        host.apply_change(modify_file(generator, gen_bar));
        assert!(
            goto_names(&host, user, other, "foo u_foo").is_empty(),
            "goto foo must fail after the generator was renamed"
        );
        assert!(
            goto_names(&host, user, other, "bar u_bar").is_empty(),
            "bar is not paid until the generator is reparsed"
        );

        let _ = host.ctx().parse_file(generator);
        assert!(
            goto_names(&host, user, other, "foo u_foo").is_empty(),
            "goto foo must stay failed after reparse"
        );
        assert_eq!(goto_names(&host, user, other, "bar u_bar"), ["bar"]);
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
    fn project_config_and_dirty_files_together_rebuild_facts() {
        let gated = "`ifdef FOO\nmodule foo;\nendmodule\n`else\nmodule bar;\nendmodule\n`endif\n";
        let other = "module other;\nendmodule\n";
        let other_id = FileId::from_raw(1);
        let mut host = AnalysisHost::default();
        host.apply_change(two_file_workspace(gated, other));
        let before = host.ctx().design_graph();
        assert!(
            before.module_names().iter().any(|name| name == "bar"),
            "{:?}",
            before.module_names()
        );
        assert!(
            !before.module_names().iter().any(|name| name == "foo"),
            "{:?}",
            before.module_names()
        );

        let mut change = project_config_with_predefines(vec!["FOO".to_owned()]);
        change.add_changed_file(vfs::ChangedFile::modify(other_id, "module other;\n  wire x;\nendmodule\n"));
        host.apply_change(change);

        let after = host.ctx().design_graph();
        assert!(
            after.module_names().iter().any(|name| name == "foo"),
            "config+dirty must recompute facts of files that were not edited: {:?}",
            after.module_names()
        );
        assert!(
            !after.module_names().iter().any(|name| name == "bar"),
            "stale unit from the old predefines must not remain: {:?}",
            after.module_names()
        );
        assert!(
            after.module_names().iter().any(|name| name == "other"),
            "{:?}",
            after.module_names()
        );
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
