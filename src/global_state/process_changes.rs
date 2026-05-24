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
use crate::config::user_config::DiagnosticsUpdateUserConfig;

#[derive(Debug)]
pub(crate) enum DiagnosticInvalidation {
    FileChanges(FxHashSet<FileId>),
    WorkspaceChanged,
}

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
        let mut deleted_file_ids = FxHashSet::default();
        for changed_file in changed_files {
            let path = vfs.file_path(changed_file.file_id);
            if let Some(path) =
                path.and_then(|path| path.as_abs_path()).map(|apath| apath.to_path_buf())
            {
                let created_or_deleted = changed_file.is_created_or_deleted();
                has_structure_changes |= created_or_deleted;
                if should_refresh_for_change(&path, created_or_deleted) {
                    workspace_structure_change = Some(path.clone());
                }
            }

            if matches!(&changed_file.change_kind, vfs::ChangeKind::Delete) {
                deleted_file_ids.insert(changed_file.file_id);
            }
            changed_file_ids.insert(changed_file.file_id);
            bytes.push(changed_file);
        }

        let mut write_guard = RwLockUpgradableReadGuard::upgrade(read_guard);
        let (vfs, line_endings_map) = &mut *write_guard;
        let change = self.collect_changes(bytes, line_endings_map, vfs, has_structure_changes);

        std::mem::drop(write_guard);

        self.analysis_host.apply_change(change);
        self.diagnostics_revision += 1;
        self.remove_deleted_qihe_diagnostics(&deleted_file_ids);
        self.clear_deleted_push_diagnostics(&deleted_file_ids);
        if has_structure_changes {
            self.invalidate_diagnostics(DiagnosticInvalidation::WorkspaceChanged);
        } else if self.config.user_config.diagnostics.update == DiagnosticsUpdateUserConfig::OnType
        {
            self.invalidate_diagnostics(DiagnosticInvalidation::FileChanges(changed_file_ids));
        }

        if let Some(path) = workspace_structure_change {
            let config = triomphe::Arc::make_mut(&mut self.config);
            config.refresh_project_manifests();
            self.request_workspace_auto_reload(format!("workspace vfs change: {:?}", path));
        }

        true
    }

    pub(crate) fn open_mem_doc_file_ids(&self) -> Vec<FileId> {
        self.mem_docs.file_ids().collect()
    }

    pub(crate) fn invalidate_diagnostics(&mut self, invalidation: DiagnosticInvalidation) {
        if self.config.cli_pull_diagnostics_support()
            && self.config.cli_workspace_diagnostic_refresh_support()
            && match &invalidation {
                DiagnosticInvalidation::FileChanges(file_ids) => !file_ids.is_empty(),
                DiagnosticInvalidation::WorkspaceChanged => true,
            }
        {
            self.send_request::<WorkspaceDiagnosticRefresh>((), DEFAULT_REQ_HANDLER);
            return;
        }

        let file_ids = match invalidation {
            DiagnosticInvalidation::FileChanges(file_ids) => {
                if self.config.diagnostics_config().semantic.enabled {
                    let snapshot = self.make_snapshot();
                    let open_file_ids =
                        self.open_mem_doc_file_ids().into_iter().collect::<FxHashSet<_>>();
                    file_ids
                        .into_iter()
                        .flat_map(|file_id| snapshot.source_root_file_ids(file_id))
                        .filter(|file_id| open_file_ids.contains(file_id))
                        .unique()
                        .collect()
                } else {
                    file_ids.into_iter().collect()
                }
            }
            DiagnosticInvalidation::WorkspaceChanged => self.open_mem_doc_file_ids(),
        };
        self.request_diagnostics(file_ids);
    }

    fn remove_deleted_qihe_diagnostics(&mut self, deleted_file_ids: &FxHashSet<FileId>) {
        if deleted_file_ids.is_empty() {
            return;
        }

        let mut qihe_diagnostics = self.qihe_diagnostics.lock();
        for file_id in deleted_file_ids {
            qihe_diagnostics.remove(file_id);
        }
    }

    fn clear_deleted_push_diagnostics(&mut self, deleted_file_ids: &FxHashSet<FileId>) {
        if deleted_file_ids.is_empty() || self.config.cli_pull_diagnostics_support() {
            return;
        }

        let snapshot = self.make_snapshot();
        let diagnostics = deleted_file_ids
            .iter()
            .filter_map(|file_id| {
                let uri = match snapshot.url(*file_id) {
                    Ok(uri) => uri,
                    Err(error) => {
                        tracing::debug!(
                            ?file_id,
                            "skipping deleted diagnostic clear for file without URI: {error:#}"
                        );
                        return None;
                    }
                };
                Some(PublishDiagnosticsTask {
                    file_id: *file_id,
                    uri,
                    version: None,
                    diagnostics: Vec::new(),
                })
            })
            .collect();

        self.publish_diagnostics_tasks(diagnostics, false);
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
            change.set_project_config(self.project_config.clone());
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
                        (change @ Delete, _, Modify(new, new_ending)) => {
                            tracing::debug!(
                                ?changed_file.file_id,
                                "received modify after delete while coalescing VFS changes"
                            );
                            *change = Modify(new, new_ending);
                        }
                        (Modify(prev, prev_ending), _, Create(new, new_ending)) => {
                            tracing::debug!(
                                ?changed_file.file_id,
                                "received create after modify while coalescing VFS changes"
                            );
                            *prev = new;
                            *prev_ending = new_ending;
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
            let mut results = Vec::with_capacity(files.len());
            for file_id in files {
                let uri = match snapshot.url(file_id) {
                    Ok(uri) => uri,
                    Err(error) => {
                        tracing::debug!(
                            ?file_id,
                            "skipping push diagnostics for file without URI: {error:#}"
                        );
                        continue;
                    }
                };
                let version = snapshot.file_version(file_id);
                let diagnostics = snapshot.lsp_diagnostics(file_id);
                results.push(PublishDiagnosticsTask { file_id, uri, version, diagnostics });
            }
            Task::Diagnostics(results)
        });
    }
}

#[cfg(test)]
mod tests {
    use lsp_server::Connection;
    use lsp_types::{ClientCapabilities, TraceValue};
    use utils::{lines::LineEnding, test_support::TestDir};
    use vfs::{VfsPath, loader::LoadResult};

    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        global_state::GlobalState,
        i18n::I18n,
    };

    #[test]
    fn ordinary_file_creation_does_not_request_workspace_reload() {
        let root = TestDir::new("ordinary-file-no-workspace-reload");
        let root_path = root.path().to_path_buf();
        let config = config::Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );
        let (server, _client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
        let file_path = root.join("top.sv");

        state.vfs.write().0.set_file_contents(
            &VfsPath::from(file_path),
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );

        assert!(state.process_changes());
        assert!(
            !state.fetch_workspaces_task.has_op_requested(),
            "loading an ordinary source file should not queue a project configuration reload"
        );
    }
}
