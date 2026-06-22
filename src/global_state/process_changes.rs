use std::collections::hash_map::Entry::{Occupied, Vacant};

use hir::base_db::change::Change;
use itertools::Itertools;
use lsp_types::request::WorkspaceDiagnosticRefresh;
use nohash_hasher::IntMap;
use parking_lot::{RwLockUpgradableReadGuard, RwLockWriteGuard};
use rustc_hash::{FxHashMap, FxHashSet};
use utils::{lines::LineEnding, thread::ThreadIntent};
use vfs::{ChangedFile, FileId, Vfs, VfsPath};

use super::{
    DEFAULT_REQ_HANDLER, GlobalState,
    diagnostics::publisher::{PublishDiagnosticsBatch, PublishDiagnosticsTask},
    reload::should_refresh_for_change,
    task::Task,
};
use crate::{config::user_config::DiagnosticsUpdateUserConfig, lsp::protocol::to_proto};

#[derive(Debug)]
pub(crate) enum DiagnosticInvalidation {
    FileChanges(FxHashSet<FileId>),
    WorkspaceChanged,
}

// Apply changes
impl GlobalState {
    pub(crate) fn process_changes(&mut self) -> bool {
        let pending_diagnostic_targets =
            std::mem::take(&mut self.diagnostics.pending_document_diagnostic_targets);
        let mut diagnostic_targets_changed = !pending_diagnostic_targets.is_empty();
        let mut write_guard = self.workspace.vfs.write();
        let changed_files = write_guard.0.take_changes();
        // downgrade earlier to allow more reader
        let read_guard = RwLockWriteGuard::downgrade_to_upgradable(write_guard);
        let vfs = &read_guard.0;
        let file_id_redirects = changed_files
            .iter()
            .filter_map(|changed_file| {
                let canonical = vfs.canonical_file_id(changed_file.file_id);
                (canonical != changed_file.file_id).then_some((changed_file.file_id, canonical))
            })
            .collect_vec();
        diagnostic_targets_changed |= !file_id_redirects.is_empty();
        for (from, to) in file_id_redirects {
            self.analysis.mem_docs.remap_file_id(from, to);
        }

        // collect changes
        let Some(changed_files) = Self::colease_modifications(changed_files) else {
            std::mem::drop(read_guard);
            if !pending_diagnostic_targets.is_empty() {
                self.diagnostics.diagnostic_target_revision += 1;
                self.request_diagnostics(pending_diagnostic_targets.into_iter().collect());
            }
            return false;
        };

        let mut workspace_structure_change = None;
        let mut has_structure_changes = false; // Any file was added or deleted
        let mut bytes = vec![];
        let mut changed_file_ids = FxHashSet::default();
        let mut content_changed_file_ids = FxHashSet::default();
        let mut deleted_file_ids = FxHashSet::default();
        let mut deleted_push_diagnostics = Vec::new();
        for changed_file in changed_files {
            let is_identity_redirect =
                vfs.canonical_file_id(changed_file.file_id) != changed_file.file_id;
            let path = if is_identity_redirect {
                vfs.original_file_path(changed_file.file_id)
            } else {
                vfs.file_path(changed_file.file_id)
            };
            if let Some(path) =
                path.and_then(|path| path.as_abs_path()).map(|apath| apath.to_path_buf())
            {
                let created_or_deleted = changed_file.is_created_or_deleted();
                has_structure_changes |= created_or_deleted;
                if !is_identity_redirect && should_refresh_for_change(&path, created_or_deleted) {
                    workspace_structure_change = Some(path.clone());
                }
            }

            if matches!(&changed_file.change_kind, vfs::ChangeKind::Delete) {
                deleted_file_ids.insert(changed_file.file_id);
                if let Some(path) = path.cloned() {
                    deleted_push_diagnostics.push((changed_file.file_id, path));
                }
            }
            changed_file_ids.insert(changed_file.file_id);
            content_changed_file_ids.insert(changed_file.file_id);
            bytes.push(changed_file);
        }
        if self.config_state.config.user_config.diagnostics.update
            == DiagnosticsUpdateUserConfig::OnType
        {
            changed_file_ids.extend(pending_diagnostic_targets.iter().copied());
        }
        let externally_changed_file_ids = content_changed_file_ids
            .iter()
            .copied()
            .filter(|file_id| !self.analysis.mem_docs.contains_file_id(*file_id))
            .collect::<FxHashSet<_>>();

        let mut write_guard = RwLockUpgradableReadGuard::upgrade(read_guard);
        let (vfs, line_endings_map) = &mut *write_guard;
        let change = self.collect_changes(bytes, line_endings_map, vfs, has_structure_changes);

        std::mem::drop(write_guard);

        self.analysis.analysis_host.apply_change(change);
        self.diagnostics.diagnostics_revision += 1;
        for file_id in &content_changed_file_ids {
            let revision = self.diagnostics.diagnostic_file_revisions.entry(*file_id).or_default();
            *revision = revision.next();
        }
        if diagnostic_targets_changed {
            self.diagnostics.diagnostic_target_revision += 1;
        }
        self.remove_deleted_qihe_diagnostics(&deleted_file_ids);
        self.clear_deleted_push_diagnostics(&deleted_push_diagnostics);
        if has_structure_changes {
            self.invalidate_diagnostics(DiagnosticInvalidation::WorkspaceChanged);
        } else {
            match self.config_state.config.user_config.diagnostics.update {
                DiagnosticsUpdateUserConfig::OnType => {
                    self.invalidate_diagnostics(DiagnosticInvalidation::FileChanges(
                        changed_file_ids,
                    ));
                }
                DiagnosticsUpdateUserConfig::OnSave if !externally_changed_file_ids.is_empty() => {
                    self.invalidate_diagnostics(DiagnosticInvalidation::FileChanges(
                        externally_changed_file_ids,
                    ));
                }
                DiagnosticsUpdateUserConfig::OnSave => {}
            }
        }
        if !pending_diagnostic_targets.is_empty()
            && (has_structure_changes
                || self.config_state.config.user_config.diagnostics.update
                    != DiagnosticsUpdateUserConfig::OnType)
        {
            self.request_diagnostics(pending_diagnostic_targets.into_iter().collect());
        }

        if let Some(path) = workspace_structure_change {
            let config = triomphe::Arc::make_mut(&mut self.config_state.config);
            config.refresh_project_manifests();
            self.request_workspace_auto_reload(format!("workspace vfs change: {:?}", path));
        }

        true
    }

    pub(crate) fn open_mem_doc_file_ids(&self) -> Vec<FileId> {
        self.analysis.mem_docs.file_ids().collect()
    }

    pub(crate) fn invalidate_diagnostics(&mut self, invalidation: DiagnosticInvalidation) {
        if !self.workspace.workspace_vfs.is_ready() {
            self.workspace.workspace_vfs.defer_diagnostics_until_ready();
            tracing::debug!(
                ?invalidation,
                "diagnostics invalidation deferred until workspace/VFS is ready"
            );
            return;
        }

        if self.config_state.config.cli_pull_diagnostics_support()
            && self.config_state.config.cli_workspace_diagnostic_refresh_support()
            && match &invalidation {
                DiagnosticInvalidation::FileChanges(file_ids) => !file_ids.is_empty(),
                DiagnosticInvalidation::WorkspaceChanged => true,
            }
        {
            self.send_request::<WorkspaceDiagnosticRefresh>((), DEFAULT_REQ_HANDLER);
            return;
        }

        let file_ids = match invalidation {
            DiagnosticInvalidation::FileChanges(file_ids) => self
                .make_snapshot()
                .diagnostic_target_file_ids_for_changes(&file_ids, self.open_mem_doc_file_ids())
                .into_iter()
                .collect(),
            DiagnosticInvalidation::WorkspaceChanged => self.open_mem_doc_file_ids(),
        };
        self.request_diagnostics(file_ids);
    }

    fn remove_deleted_qihe_diagnostics(&mut self, deleted_file_ids: &FxHashSet<FileId>) {
        if deleted_file_ids.is_empty() {
            return;
        }

        let mut qihe_diagnostics = self.qihe.qihe_diagnostics.lock();
        for file_id in deleted_file_ids {
            qihe_diagnostics.remove(file_id);
        }
    }

    fn clear_deleted_push_diagnostics(&mut self, deleted_files: &[(FileId, VfsPath)]) {
        if deleted_files.is_empty() || self.config_state.config.cli_pull_diagnostics_support() {
            return;
        }

        let diagnostics = deleted_files
            .iter()
            .filter_map(|(file_id, path)| {
                let Some(path) = path.as_abs_path() else {
                    tracing::debug!(
                        ?file_id,
                        ?path,
                        "skipping deleted diagnostic clear for non-file path"
                    );
                    return None;
                };
                let uri = match to_proto::url_from_abs_path(path) {
                    Ok(uri) => uri,
                    Err(error) => {
                        tracing::debug!(
                            ?file_id,
                            ?path,
                            "skipping deleted diagnostic clear for file without URI: {error:#}"
                        );
                        return None;
                    }
                };
                Some(PublishDiagnosticsTask::clear_stale_uri(*file_id, uri))
            })
            .collect();

        self.publish_diagnostics_tasks(PublishDiagnosticsBatch::from_tasks(
            diagnostics,
            self.diagnostic_publish_freshness(),
        ));
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
            let roots = self.config_state.source_root_config.partition(vfs);
            change.set_roots(roots);
            change.set_project_config(self.config_state.project_config.clone());
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

        if !self.workspace.workspace_vfs.is_ready() {
            self.workspace.workspace_vfs.defer_diagnostics_until_ready();
            tracing::debug!(
                file_count = files.len(),
                "diagnostics request deferred until workspace/VFS is ready"
            );
            return;
        }

        if self.config_state.config.cli_pull_diagnostics_support() {
            if self.config_state.config.cli_workspace_diagnostic_refresh_support() {
                self.send_request::<WorkspaceDiagnosticRefresh>((), DEFAULT_REQ_HANDLER);
            }
            return;
        }

        let snapshot = self.make_snapshot();
        self.tasks.task_pool.handle.spawn_and_send(ThreadIntent::Worker, move || {
            let mut results = Vec::with_capacity(files.len());
            let mut touched_file_ids = FxHashSet::default();
            for file_id in files {
                let targets = match snapshot.diagnostic_publish_targets(file_id) {
                    Ok(targets) => targets,
                    Err(error) => {
                        tracing::debug!(
                            ?file_id,
                            "skipping push diagnostics for file without URI: {error:#}"
                        );
                        continue;
                    }
                };
                let diagnostics = match snapshot.lsp_diagnostics(file_id) {
                    Ok(diagnostics) => diagnostics,
                    Err(error) if error.is::<ide::Cancelled>() => {
                        tracing::debug!(?file_id, "diagnostics computation cancelled");
                        continue;
                    }
                    Err(error) => {
                        tracing::debug!(?file_id, "diagnostics computation failed: {error:#}");
                        continue;
                    }
                };
                touched_file_ids.insert(file_id);
                results.extend(targets.into_iter().map(|target| {
                    PublishDiagnosticsTask::from_target(target, diagnostics.clone())
                }));
            }
            Task::Diagnostics(PublishDiagnosticsBatch::for_touched_files(
                touched_file_ids,
                results,
                snapshot.diagnostic_publish_freshness,
            ))
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
                process_name: "vide-test".to_string(),
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

        state.workspace.vfs.write().0.set_file_contents(
            &VfsPath::from(file_path),
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );

        assert!(state.process_changes());
        assert!(
            !state.workspace.fetch_workspaces_task.has_op_requested(),
            "loading an ordinary source file should not queue a project configuration reload"
        );
    }
}
