use std::collections::hash_map::Entry::{Occupied, Vacant};

use base_db::change::Change;
use itertools::Itertools;
use lsp_types::request::WorkspaceDiagnosticRefresh;
use nohash_hasher::IntMap;
use parking_lot::{RwLockUpgradableReadGuard, RwLockWriteGuard};
use rustc_hash::{FxHashMap, FxHashSet};
use utils::{lines::LineEnding, thread::ThreadIntent};
use vfs::{ChangedFile, FileId, Vfs};

use super::{
    DEFAULT_REQ_HANDLER, GlobalState,
    main_loop::{PublishDiagnosticsTask, Task},
    reload::should_refresh_for_change,
};

// Apply changes
impl GlobalState {
    pub(crate) fn process_changes(&mut self) -> bool {
        let mut write_guard = self.vfs.write();
        let changed_files = write_guard.0.take_changes();
        // downgrade earlier to allow more reader
        let read_guard = RwLockWriteGuard::downgrade_to_upgradable(write_guard);
        let vfs = &read_guard.0;

        // collect changes
        let Some(changed_files) = Self::colease_modifications(changed_files) else {
            return false;
        };

        let mut workspace_structure_change = None;
        let mut has_structure_changes = false; // Any file was added or deleted
        let mut bytes = vec![];
        let mut changed_file_ids = FxHashSet::default();
        for changed_file in changed_files {
            let path = vfs.file_path(changed_file.file_id);
            if let Some(path) = path.as_abs_path().map(|apath| apath.to_path_buf()) {
                let created_or_deleted = changed_file.is_created_or_deleted();
                has_structure_changes |= created_or_deleted;
                if created_or_deleted || should_refresh_for_change(&path, created_or_deleted) {
                    workspace_structure_change = Some(path.clone());
                }
            }

            changed_file_ids.insert(changed_file.file_id);
            bytes.push(changed_file);
        }

        let mut write_guard = RwLockUpgradableReadGuard::upgrade(read_guard);
        let (vfs, line_endings_map) = &mut *write_guard;
        let change = self.collect_changes(bytes, line_endings_map, vfs, has_structure_changes);

        std::mem::drop(write_guard);

        self.analysis_host.apply_change(change);
        self.request_diagnostics(changed_file_ids.into_iter().collect());

        if let Some(path) = workspace_structure_change {
            self.fetch_workspaces_task.request(format!("workspace vfs change: {:?}", path));
        }

        true
    }

    fn collect_changes(
        &self,
        bytes: Vec<ChangedFile>,
        line_ending_map: &mut IntMap<FileId, LineEnding>,
        vfs: &mut Vfs,
        has_structure_changes: bool,
    ) -> Change {
        let mut change = Change::new();
        for changed_file in bytes {
            let file_id = changed_file.file_id;
            if let Some(line_ending) = changed_file.ending() {
                line_ending_map.insert(file_id, line_ending);
            }
            change.add_changed_file(changed_file)
        }
        if has_structure_changes {
            let roots = self.source_root_config.partition(vfs);
            change.set_roots(roots);
        }
        change
    }

    fn colease_modifications(vfs_changes: Vec<ChangedFile>) -> Option<Vec<ChangedFile>> {
        if vfs_changes.is_empty() {
            return None;
        }

        // collapse modifications
        use vfs::ChangeKind::*;

        let mut file_changes = FxHashMap::default();
        for changed_file in vfs_changes {
            match file_changes.entry(changed_file.file_id) {
                Occupied(mut entry) => {
                    let (change, just_created) = entry.get_mut();

                    match (change, *just_created, changed_file.change_kind) {
                        (change, _, Delete) => *change = Delete,
                        (
                            Create(prev, prev_ending),
                            _,
                            Create(new, new_ending) | Modify(new, new_ending),
                        ) => {
                            *prev = new;
                            *prev_ending = new_ending;
                        }
                        (Modify(prev, prev_ending), _, Modify(new, new_ending)) => {
                            *prev = new;
                            *prev_ending = new_ending;
                        }
                        (change @ Delete, _, Create(new, new_ending)) => {
                            *change = Modify(new, new_ending);
                            *just_created = true;
                        }
                        (Delete, _, Modify(_, _)) | (Modify(_, _), _, Create(_, _)) => {
                            unreachable!()
                        }
                    }
                }
                Vacant(v) => {
                    let just_created = matches!(&changed_file.change_kind, Create(_, _));
                    v.insert((changed_file.change_kind, just_created));
                }
            }
        }

        let changed_file = file_changes
            .into_iter()
            .filter(|(_, (change_kind, just_created))| {
                !(*just_created && matches!(change_kind, Delete))
            })
            .map(|(file_id, (change_kind, _))| ChangedFile { file_id, change_kind })
            .collect_vec();

        Some(changed_file)
    }

    pub(crate) fn request_diagnostics(&mut self, files: Vec<FileId>) {
        if files.is_empty() {
            return;
        }

        if self.config.cli_pull_diagnostics_support() {
            if self.config.cli_workspace_diagnostic_refresh_support() {
                self.send_request::<WorkspaceDiagnosticRefresh>((), DEFAULT_REQ_HANDLER);
            }
            return;
        }

        let snapshot = self.make_snapshot();
        self.task_pool.handle.spawn_and_send(ThreadIntent::Worker, move || {
            let results = files
                .into_iter()
                .map(|file_id| {
                    let uri = snapshot.url(file_id);
                    let version = snapshot.file_version(file_id);
                    let diagnostics = snapshot.lsp_diagnostics(file_id);

                    PublishDiagnosticsTask { file_id, uri, version, diagnostics }
                })
                .collect();
            Task::Diagnostics(results)
        });
    }
}
