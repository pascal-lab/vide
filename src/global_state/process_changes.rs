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
    main_loop::{PublishDiagnosticsBatch, PublishDiagnosticsTask, Task},
    reload::should_refresh_for_change,
};
use crate::{config::user_config::DiagnosticsUpdateUserConfig, lsp_ext::to_proto};

#[derive(Debug)]
pub(crate) enum DiagnosticInvalidation {
    FileChanges(FxHashSet<FileId>),
    WorkspaceChanged,
}

// Apply changes
impl GlobalState {
    pub(crate) fn process_changes(&mut self) -> bool {
        let pending_diagnostic_targets =
            std::mem::take(&mut self.pending_document_diagnostic_targets);
        let mut diagnostic_targets_changed = !pending_diagnostic_targets.is_empty();
        let mut write_guard = self.vfs.write();
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
            self.mem_docs.remap_file_id(from, to);
        }

        // collect changes
        let Some(changed_files) = Self::colease_modifications(changed_files) else {
            std::mem::drop(read_guard);
            if !pending_diagnostic_targets.is_empty() {
                self.diagnostic_target_revision += 1;
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
        if self.config.user_config.diagnostics.update == DiagnosticsUpdateUserConfig::OnType {
            changed_file_ids.extend(pending_diagnostic_targets.iter().copied());
        }
        let externally_changed_file_ids = content_changed_file_ids
            .iter()
            .copied()
            .filter(|file_id| !self.mem_docs.contains_file_id(*file_id))
            .collect::<FxHashSet<_>>();

        let mut write_guard = RwLockUpgradableReadGuard::upgrade(read_guard);
        let (vfs, line_endings_map) = &mut *write_guard;
        let change = self.collect_changes(bytes, line_endings_map, vfs, has_structure_changes);

        std::mem::drop(write_guard);

        self.analysis_host.apply_change(change);
        self.diagnostics_revision += 1;
        for file_id in &content_changed_file_ids {
            let revision = self.diagnostic_file_revisions.entry(*file_id).or_default();
            *revision = revision.next();
        }
        if diagnostic_targets_changed {
            self.diagnostic_target_revision += 1;
        }
        self.remove_deleted_qihe_diagnostics(&deleted_file_ids);
        self.clear_deleted_push_diagnostics(&deleted_push_diagnostics);
        if has_structure_changes {
            self.invalidate_diagnostics(DiagnosticInvalidation::WorkspaceChanged);
        } else {
            match self.config.user_config.diagnostics.update {
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
                || self.config.user_config.diagnostics.update
                    != DiagnosticsUpdateUserConfig::OnType)
        {
            self.request_diagnostics(pending_diagnostic_targets.into_iter().collect());
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
        if !self.workspace_vfs.is_ready() {
            self.workspace_vfs.defer_diagnostics_until_ready();
            tracing::debug!(
                ?invalidation,
                "diagnostics invalidation deferred until workspace/VFS is ready"
            );
            return;
        }

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

        let mut qihe_diagnostics = self.qihe_diagnostics.lock();
        for file_id in deleted_file_ids {
            qihe_diagnostics.remove(file_id);
        }
    }

    fn clear_deleted_push_diagnostics(&mut self, deleted_files: &[(FileId, VfsPath)]) {
        if deleted_files.is_empty() || self.config.cli_pull_diagnostics_support() {
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

        if !self.workspace_vfs.is_ready() {
            self.workspace_vfs.defer_diagnostics_until_ready();
            tracing::debug!(
                file_count = files.len(),
                "diagnostics request deferred until workspace/VFS is ready"
            );
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
    use std::{fs, time::Duration};

    use lsp_server::Connection;
    use lsp_types::{
        ClientCapabilities, Diagnostic, DiagnosticSeverity, DidCloseTextDocumentParams,
        DidOpenTextDocumentParams, Position, PublishDiagnosticsParams, Range,
        TextDocumentIdentifier, TextDocumentItem, TraceValue, notification::Notification,
    };
    use rustc_hash::FxHashSet;
    use utils::{lines::LineEnding, test_support::TestDir};
    use vfs::{VfsPath, loader::LoadResult};

    use crate::{
        Opt,
        config::{
            self,
            user_config::{DiagnosticsUpdateUserConfig, UserConfig},
        },
        global_state::{
            GlobalState,
            handlers::notification::{
                handle_did_close_text_document, handle_did_open_text_document,
            },
            main_loop::{
                DiagnosticPublishKey, PublishDiagnosticsBatch, PublishDiagnosticsTask, Task,
            },
        },
        i18n::I18n,
        lsp_ext::to_proto,
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

    #[test]
    fn identity_redirect_delete_clears_duplicate_uri_not_canonical_uri() {
        let root = TestDir::new("identity-redirect-delete-diagnostics");
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
        let (server, client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
        let source = root.join("workspace/top.sv");
        let alias = root.join("alias/top.sv");
        let source_vfs_path = VfsPath::from(source.clone());
        let alias_vfs_path = VfsPath::from(alias.clone());

        {
            let mut vfs = state.vfs.write();
            vfs.0.set_file_contents(
                &source_vfs_path,
                LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
            );
            vfs.0.set_file_contents(
                &alias_vfs_path,
                LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
            );
        }
        assert!(state.process_changes());
        let source_file_id = state.vfs.read().0.file_id(&source_vfs_path).unwrap();
        let alias_file_id = state.vfs.read().0.file_id(&alias_vfs_path).unwrap();
        assert_ne!(source_file_id, alias_file_id);
        let alias_uri = to_proto::url_from_abs_path(alias.as_path()).unwrap();

        state.published_diagnostics.insert(
            DiagnosticPublishKey::for_test(alias_file_id, alias_uri.clone()),
            vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("test".to_owned()),
                message: "stale alias diagnostic".to_owned(),
                ..Diagnostic::default()
            }],
        );

        if let Some(parent) = source.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        if let Some(parent) = alias.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&source, "module top; endmodule\n").unwrap();
        fs::hard_link(&source, &alias).unwrap();
        {
            let mut vfs = state.vfs.write();
            vfs.0.set_file_contents(
                &source_vfs_path,
                LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
            );
            vfs.0.set_file_contents(
                &alias_vfs_path,
                LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
            );
        }

        assert!(state.process_changes());

        let message = client.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let lsp_server::Message::Notification(notification) = message else {
            panic!("expected publishDiagnostics notification");
        };
        assert_eq!(notification.method, lsp_types::notification::PublishDiagnostics::METHOD);
        let params: PublishDiagnosticsParams = serde_json::from_value(notification.params).unwrap();
        let source_uri = to_proto::url_from_abs_path(source.as_path()).unwrap();
        assert_eq!(params.uri, alias_uri);
        assert_ne!(params.uri, source_uri);
        assert!(params.diagnostics.is_empty());
    }

    #[test]
    fn opening_alias_requests_diagnostics_for_every_open_uri() {
        let root = TestDir::new("diagnostics-target-open-alias");
        let root_path = root.path().to_path_buf();
        let mut user_config = UserConfig::default();
        user_config.diagnostics.update = DiagnosticsUpdateUserConfig::OnType;
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
            user_config,
            Vec::new(),
        );
        let (server, client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
        let source = root.write("workspace/top.sv", "module top; endmodule\n");
        let alias = root.join("alias/top.sv");
        if let Some(parent) = alias.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::hard_link(&source, &alias).unwrap();
        let source_vfs_path = VfsPath::from(source.clone());
        let source_uri = to_proto::url_from_abs_path(source.as_path()).unwrap();
        let alias_uri = to_proto::url_from_abs_path(alias.as_path()).unwrap();

        state.vfs.write().0.set_file_contents(
            &source_vfs_path,
            LoadResult::Loaded("module top; endmodule\n".to_owned(), LineEnding::Unix),
        );
        assert!(state.process_changes());
        let file_id = state.vfs.read().0.file_id(&source_vfs_path).unwrap();

        handle_did_open_text_document(
            &mut state,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: source_uri.clone(),
                    language_id: "systemverilog".to_owned(),
                    version: 3,
                    text: "module top; endmodule\n".to_owned(),
                },
            },
        )
        .unwrap();
        assert!(!state.process_changes());
        let Task::Diagnostics(batch) =
            state.task_pool.receiver.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected diagnostics task");
        };
        let tasks = batch.into_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].uri(), &source_uri);
        assert_eq!(tasks[0].version(), Some(3));

        handle_did_open_text_document(
            &mut state,
            DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: alias_uri.clone(),
                    language_id: "systemverilog".to_owned(),
                    version: 12,
                    text: "module top; endmodule\n".to_owned(),
                },
            },
        )
        .unwrap();

        assert!(!state.process_changes());
        let task = state.task_pool.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let Task::Diagnostics(batch) = task else {
            panic!("expected diagnostics task");
        };
        let mut tasks = batch.into_tasks();
        tasks.sort_unstable_by(|lhs, rhs| lhs.uri().cmp(rhs.uri()));
        let targets = tasks
            .into_iter()
            .map(|task| {
                assert_eq!(task.file_id(), file_id);
                (task.uri().clone(), task.version())
            })
            .collect::<Vec<_>>();
        let mut expected = vec![(source_uri.clone(), Some(3)), (alias_uri.clone(), Some(12))];
        expected.sort_unstable_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
        assert_eq!(targets, expected);
        assert_eq!(
            &*state.make_snapshot().file_text(file_id).unwrap(),
            "module top; endmodule\n",
            "second open URI aliases the same FileId but must not replace the canonical analysis buffer"
        );
        let stale_alias_batch = PublishDiagnosticsBatch::for_touched_files(
            FxHashSet::from_iter([file_id]),
            vec![PublishDiagnosticsTask::for_test(
                file_id,
                alias_uri.clone(),
                Some(12),
                vec![Diagnostic {
                    range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("test".to_owned()),
                    message: "stale alias diagnostic".to_owned(),
                    ..Diagnostic::default()
                }],
            )],
            state.diagnostic_publish_freshness(),
        );
        state.published_diagnostics.insert(
            DiagnosticPublishKey::for_test(file_id, alias_uri.clone()),
            vec![Diagnostic {
                range: Range::new(Position::new(0, 0), Position::new(0, 1)),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("test".to_owned()),
                message: "stale alias diagnostic".to_owned(),
                ..Diagnostic::default()
            }],
        );

        handle_did_close_text_document(
            &mut state,
            DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: alias_uri.clone() },
            },
        )
        .unwrap();

        assert!(!state.process_changes());
        let task = state.task_pool.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let Task::Diagnostics(batch) = task else {
            panic!("expected diagnostics task");
        };
        assert_eq!(batch.touched_file_ids(), &FxHashSet::from_iter([file_id]));
        assert_eq!(batch.tasks().len(), 1);
        assert_eq!(batch.tasks()[0].file_id(), file_id);
        assert_eq!(batch.tasks()[0].uri(), &source_uri);
        assert_eq!(batch.tasks()[0].version(), Some(3));
        state.publish_diagnostics_tasks(batch);
        let message = client.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let lsp_server::Message::Notification(notification) = message else {
            panic!("expected publishDiagnostics notification");
        };
        assert_eq!(notification.method, lsp_types::notification::PublishDiagnostics::METHOD);
        let params: PublishDiagnosticsParams = serde_json::from_value(notification.params).unwrap();
        assert_eq!(params.uri, alias_uri);
        assert!(params.diagnostics.is_empty());
        state.publish_diagnostics_tasks(stale_alias_batch);
        assert!(
            client.receiver.recv_timeout(Duration::from_millis(50)).is_err(),
            "stale target batch must not republish diagnostics to a closed alias URI"
        );
        assert!(
            !state
                .published_diagnostics
                .contains_key(&DiagnosticPublishKey::for_test(file_id, alias_uri))
        );
    }
}
