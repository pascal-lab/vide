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
        let (store, affected_files) = ProductStore::transition(&self.store, &mut self.db, change);
        self.store = store;
        self.advance_revision();
        if !affected_files.is_empty() {
            self.start_prewarm(affected_files);
        }
    }

    /// Apply a change without starting revision prewarm. Benches that build
    /// a large workspace would otherwise spend Drop joining `unit_scope`
    /// over every file.
    #[cfg(test)]
    pub(crate) fn apply_change_without_prewarm(&mut self, change: Change) {
        self.cancel_prewarm();
        let (store, _) = ProductStore::transition(&self.store, &mut self.db, change);
        self.store = store;
        self.advance_revision();
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
                if worker_cancel.load(Ordering::Acquire) {
                    return;
                }
                let ctx = AnalysisContext { db: &db, store: &store };
                for file_id in affected_files {
                    if worker_cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if db.file_kind(file_id).is_semantic_compilation_unit() {
                        let _ = <dyn design_graph::DesignGraphDb>::file_decls(&db, file_id);
                    }
                }
                let _ = ctx.prewarm_unit_catalog(&worker_cancel);
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
        use base_db::{
            project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
            source_root::SourceRootId,
        };
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
        let before = host.ctx().unit_catalog();
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
        let after_edit = host.ctx().unit_catalog();
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
        let after_reparse = host.ctx().unit_catalog();
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
        let first = host.ctx().unit_catalog();
        assert_eq!(first.node_count(), 1);
        assert!(first.module_names().iter().any(|name| name == "first"));

        host.apply_change(add_second_file("module second;\nendmodule\n"));
        let both = host.ctx().unit_catalog();
        assert_eq!(both.node_count(), 2);
        assert!(both.module_names().iter().any(|name| name == "first"));
        assert!(both.module_names().iter().any(|name| name == "second"));
    }

    #[test]
    fn file_decls_backdate_across_a_body_only_edit() {
        use std::cell::Cell;

        use design_graph::DesignGraphDb;
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text("module first;\nendmodule\n"));
        let file = FileId::from_raw(0);
        let before_decls = <dyn DesignGraphDb>::file_decls(host.ctx().db, file);
        design_graph::db::SOURCE_CATALOG_RUNS.with(|runs| runs.set(0));
        let before = <dyn DesignGraphDb>::source_unit_catalog(host.ctx().db);
        let runs_after_first = design_graph::db::SOURCE_CATALOG_RUNS.with(Cell::get);
        host.apply_change(modify_with_file_text("module first;\n  wire x;\nendmodule\n"));
        let after_decls = <dyn DesignGraphDb>::file_decls(host.ctx().db, file);
        let after = <dyn DesignGraphDb>::source_unit_catalog(host.ctx().db);
        let runs_after_edit = design_graph::db::SOURCE_CATALOG_RUNS.with(Cell::get);
        assert_eq!(
            *before_decls, *after_decls,
            "position-free decls must be value-equal after a body-only edit"
        );
        assert_eq!(before.as_ref(), after.as_ref());
        // Body-only edits leave `file_decls` value-equal. Salsa must
        // backdate the L0 catalog rather than re-fold it. An extra
        // `set_file_kind` on the same enum dirties every query that
        // reads kind, and looks like a backdating failure.
        assert_eq!(
            runs_after_edit, runs_after_first,
            "salsa catalog must not re-execute after a body-only edit (first={runs_after_first} after={runs_after_edit})"
        );
    }

    #[test]
    fn generated_overlay_is_outside_the_salsa_source_catalog() {
        use std::cell::Cell;

        use design_graph::DesignGraphDb;
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text(
            "`define GEN(name) module name; endmodule\n`GEN(foo)\nmodule top;\nendmodule\n",
        ));
        design_graph::db::SOURCE_CATALOG_RUNS.with(|runs| runs.set(0));
        let source_before = <dyn DesignGraphDb>::source_unit_catalog(host.ctx().db);
        let runs_before_parse = design_graph::db::SOURCE_CATALOG_RUNS.with(Cell::get);
        assert!(
            source_before.module_names().iter().any(|name| name == "top"),
            "{:?}",
            source_before.module_names()
        );
        assert!(
            !source_before.module_names().iter().any(|name| name == "foo"),
            "L0 salsa catalog must not see a generated name: {:?}",
            source_before.module_names()
        );

        let _ = host.ctx().parse_file(FileId::from_raw(0));
        let source_after = <dyn DesignGraphDb>::source_unit_catalog(host.ctx().db);
        let runs_after_parse = design_graph::db::SOURCE_CATALOG_RUNS.with(Cell::get);
        let production = host.ctx().unit_catalog();
        assert_eq!(
            runs_after_parse, runs_before_parse,
            "recording generated units must not re-execute the salsa catalog (before={runs_before_parse} after={runs_after_parse})"
        );
        assert!(
            !source_after.module_names().iter().any(|name| name == "foo"),
            "{:?}",
            source_after.module_names()
        );
        assert!(
            production.module_names().iter().any(|name| name == "foo"),
            "production catalog merges the overlay salsa cannot see: {:?}",
            production.module_names()
        );
        assert!(
            production.module_names().iter().any(|name| name == "top"),
            "{:?}",
            production.module_names()
        );
        let overlay = host.ctx().store.generated_units(host.ctx().db);
        assert_eq!(
            production.as_ref(),
            &source_after.with_overlay(&overlay),
            "production catalog is the salsa source plus the current overlay"
        );
    }

    #[test]
    fn body_only_edit_keeps_the_design_graph_nodes() {
        let mut host = AnalysisHost::default();
        host.apply_change(change_with_file_text("module first;\nendmodule\n"));
        let before = host.ctx().unit_catalog();
        assert_eq!(before.node_count(), 1);

        host.apply_change(modify_with_file_text("module first;\n  wire x;\nendmodule\n"));
        let after = host.ctx().unit_catalog();
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
        let before = host.ctx().unit_catalog();
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
        change.add_changed_file(vfs::ChangedFile::modify(
            other_id,
            "module other;\n  wire x;\nendmodule\n",
        ));
        host.apply_change(change);

        let after = host.ctx().unit_catalog();
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
