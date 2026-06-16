use lsp_server::{Notification, Request, Response};
use lsp_types::request::Request as _;
use project_model::project_manifest;

pub use super::event_loop::main_loop;
use super::{
    GlobalState,
    dispatcher::{NotifDispatcher, ReqDispatcher},
    handlers,
    reload::FetchWorkspaceProgress,
    task::{ResponseTask, Task},
};
use crate::{global_state::DEFAULT_REQ_HANDLER, i18n::keys};

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lsp_server::{Connection, Message};
    use lsp_types::{
        ClientCapabilities, Diagnostic, DiagnosticSeverity, Position, ProgressParams,
        ProgressParamsValue, PublishDiagnosticsParams, Range, WindowClientCapabilities,
        WorkDoneProgress, notification::Notification as _, request::Request as _,
    };
    use project_model::{ProjectModel, project_manifest::ProjectManifest};
    use rustc_hash::FxHashSet;
    use triomphe::Arc;
    use utils::{lines::LineEnding, paths::AbsPathBuf, test_support::TestDir};
    use vfs::{FileId, VfsPath, loader as vfs_loader, loader::LoadResult};

    use super::*;
    use crate::{
        Opt,
        config::{Config, user_config::UserConfig},
        global_state::{
            diagnostics::{
                DiagnosticPublishFreshness,
                publisher::{
                    DiagnosticPublishKey, PublishDiagnosticsBatch, PublishDiagnosticsTask,
                },
            },
            event_loop::Event,
            response_effect::AcceptedResponseEffect,
        },
        i18n::I18n,
        lsp_ext::to_proto,
    };

    fn test_state_with_caps(
        root_path: AbsPathBuf,
        client_caps: ClientCapabilities,
    ) -> (GlobalState, Connection) {
        let config = Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            client_caps,
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );

        let (server, client) = Connection::memory();
        (GlobalState::new(server.sender, config, lsp_types::TraceValue::Off), client)
    }

    fn test_state(root_path: AbsPathBuf) -> GlobalState {
        test_state_with_caps(root_path, ClientCapabilities::default()).0
    }

    fn workspace_model(root_path: AbsPathBuf) -> Vec<project_model::Workspace> {
        let (model, errors) =
            ProjectModel::load(vec![ProjectManifest::UnconfiguredRoot(root_path)]);
        assert!(errors.is_empty(), "{errors:#?}");
        model.workspaces
    }

    fn recv_publish(client: &Connection) -> PublishDiagnosticsParams {
        let message = client.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let lsp_server::Message::Notification(notification) = message else {
            panic!("expected publishDiagnostics notification");
        };
        assert_eq!(notification.method, lsp_types::notification::PublishDiagnostics::METHOD);
        serde_json::from_value(notification.params).unwrap()
    }

    fn recv_work_done_progress(client: &Connection) -> WorkDoneProgress {
        for _ in 0..8 {
            let message = client.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
            if let Message::Notification(notification) = message
                && notification.method == lsp_types::notification::Progress::METHOD
            {
                let params: ProgressParams = serde_json::from_value(notification.params).unwrap();
                let ProgressParamsValue::WorkDone(progress) = params.value;
                return progress;
            }
        }
        panic!("expected work-done progress notification");
    }

    fn publish_batch(tasks: Vec<PublishDiagnosticsTask>) -> PublishDiagnosticsBatch {
        PublishDiagnosticsBatch::from_tasks(tasks, DiagnosticPublishFreshness::default())
    }

    #[test]
    fn publish_diagnostics_cache_is_scoped_by_file_and_uri() {
        let root = TestDir::new("diagnostics-cache-by-uri");
        let root_path = root.path().to_path_buf();
        let config = Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            lsp_types::ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );
        let (server, client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, lsp_types::TraceValue::Off);
        let file_id = FileId(0);
        let primary_uri =
            to_proto::url_from_abs_path(root.write("workspace/top.sv", "").as_path()).unwrap();
        let alias_uri =
            to_proto::url_from_abs_path(root.write("alias/top.sv", "").as_path()).unwrap();
        let diagnostic = Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("test".to_owned()),
            message: "same diagnostic".to_owned(),
            ..Diagnostic::default()
        };

        state.publish_diagnostics_tasks(publish_batch(vec![PublishDiagnosticsTask::for_test(
            file_id,
            primary_uri.clone(),
            None,
            vec![diagnostic.clone()],
        )]));
        let first = recv_publish(&client);
        assert_eq!(first.uri, primary_uri);
        assert_eq!(first.diagnostics, vec![diagnostic.clone()]);

        state.publish_diagnostics_tasks(publish_batch(vec![PublishDiagnosticsTask::for_test(
            file_id,
            alias_uri.clone(),
            Some(7),
            vec![diagnostic.clone()],
        )]));

        let clear_primary = recv_publish(&client);
        assert_eq!(clear_primary.uri, primary_uri);
        assert!(clear_primary.diagnostics.is_empty());
        let publish_alias = recv_publish(&client);
        assert_eq!(publish_alias.uri, alias_uri);
        assert_eq!(publish_alias.version, Some(7));
        assert_eq!(publish_alias.diagnostics, vec![diagnostic]);
        assert!(
            state
                .published_diagnostics
                .contains_key(&DiagnosticPublishKey::for_test(file_id, alias_uri))
        );
    }

    #[test]
    fn publish_diagnostics_clears_stale_targets_when_target_set_is_empty() {
        let root = TestDir::new("diagnostics-clear-empty-target-set");
        let root_path = root.path().to_path_buf();
        let config = Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            lsp_types::ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );
        let (server, client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, lsp_types::TraceValue::Off);
        let file_id = FileId(0);
        let alias_uri =
            to_proto::url_from_abs_path(root.write("alias/top.sv", "").as_path()).unwrap();
        let diagnostic = Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("test".to_owned()),
            message: "stale alias diagnostic".to_owned(),
            ..Diagnostic::default()
        };

        state.publish_diagnostics_tasks(publish_batch(vec![PublishDiagnosticsTask::for_test(
            file_id,
            alias_uri.clone(),
            Some(9),
            vec![diagnostic],
        )]));
        let published = recv_publish(&client);
        assert_eq!(published.uri, alias_uri);
        assert!(!published.diagnostics.is_empty());

        state.publish_diagnostics_tasks(PublishDiagnosticsBatch::for_touched_files(
            FxHashSet::from_iter([file_id]),
            Vec::new(),
            DiagnosticPublishFreshness::default(),
        ));

        let cleared = recv_publish(&client);
        assert_eq!(cleared.uri, alias_uri);
        assert!(cleared.diagnostics.is_empty());
        assert!(
            !state
                .published_diagnostics
                .contains_key(&DiagnosticPublishKey::for_test(file_id, alias_uri))
        );
    }

    #[test]
    fn stale_diagnostics_batch_does_not_publish() {
        let root = TestDir::new("stale-diagnostics-batch");
        let root_path = root.path().to_path_buf();
        let config = Config::new(
            Opt {
                process_name: "vide-test".to_string(),
                log: "error".to_string(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            lsp_types::ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );
        let (server, client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, lsp_types::TraceValue::Off);
        state.diagnostics_revision = 2;
        let file_id = FileId(0);
        let uri =
            to_proto::url_from_abs_path(root.write("workspace/top.sv", "").as_path()).unwrap();
        let diagnostic = Diagnostic {
            range: Range::new(Position::new(0, 0), Position::new(0, 1)),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("test".to_owned()),
            message: "stale diagnostic".to_owned(),
            ..Diagnostic::default()
        };

        state.publish_diagnostics_tasks(PublishDiagnosticsBatch::from_tasks(
            vec![PublishDiagnosticsTask::for_test(file_id, uri, None, vec![diagnostic])],
            DiagnosticPublishFreshness::new(1, 0, 0),
        ));

        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(state.published_diagnostics.is_empty());
    }

    #[test]
    fn stale_loaded_batches_do_not_update_vfs() {
        let root = TestDir::new("stale-loaded-batches");
        let root_path = root.path().to_path_buf();
        let file_path = root_path.join("stale.sv");
        let mut state = test_state(root_path);
        state.workspace_vfs.begin_vfs_load(1);
        state.workspace_vfs.begin_vfs_load(1);

        state.process_vfs_msg(vfs_loader::Message::Loaded {
            files: vec![(
                file_path.clone(),
                LoadResult::Loaded("module stale; endmodule\n".to_string(), LineEnding::Unix),
            )],
            config_version: 1,
        });

        let vfs_path = VfsPath::from(file_path);
        let mut vfs = state.vfs.write();
        assert!(vfs.0.file_id(&vfs_path).is_none());
        assert!(vfs.0.take_changes().is_empty());
    }

    #[test]
    fn empty_vfs_load_waits_for_loader_ack() {
        let root = TestDir::new("empty-vfs-load-waits-for-ack");
        let root_path = root.path().to_path_buf();
        let (mut state, client) = test_state_with_caps(
            root_path,
            ClientCapabilities {
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..WindowClientCapabilities::default()
                }),
                ..ClientCapabilities::default()
            },
        );

        let config_version = state.workspace_vfs.begin_vfs_load(0);
        assert!(!state.workspace_vfs.is_ready());

        state.process_vfs_msg(vfs_loader::Message::Progress {
            n_total: 0,
            n_done: 0,
            config_version,
        });

        assert!(state.workspace_vfs.is_ready());
        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());
    }

    #[test]
    fn diagnostic_requests_are_parked_until_workspace_ready() {
        let root = TestDir::new("diagnostic-request-readiness-queue");
        let root_path = root.path().to_path_buf();
        let mut state = test_state(root_path);
        let config_version = state.workspace_vfs.begin_vfs_load(1);
        let request_id = lsp_server::RequestId::from(7);
        let req = Request::new(
            request_id.clone(),
            lsp_types::request::WorkspaceDiagnosticRequest::METHOD.to_owned(),
            lsp_types::WorkspaceDiagnosticParams {
                identifier: None,
                previous_result_ids: Vec::new(),
                work_done_progress_params: lsp_types::WorkDoneProgressParams::default(),
                partial_result_params: Default::default(),
            },
        );

        state.register_request(Instant::now(), &req);
        state.handle_request(req);

        assert_eq!(state.pending_diagnostic_requests.len(), 1);
        assert!(state.task_pool.receiver.recv_timeout(Duration::from_millis(50)).is_err());

        state
            .handle_event(Event::Vfs(vfs_loader::Message::Progress {
                n_total: 1,
                n_done: 1,
                config_version,
            }))
            .unwrap();

        assert!(state.pending_diagnostic_requests.is_empty());
        let task = state.task_pool.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let Task::Response(response) = task else {
            panic!("expected parked diagnostic request to resume as response task, got {task:?}");
        };
        assert_eq!(response.response.id, request_id);
    }

    #[test]
    fn accepted_response_effects_commit_only_after_response_acceptance() {
        let root = TestDir::new("accepted-response-effects");
        let root_path = root.path().to_path_buf();
        let (mut state, _client) = test_state_with_caps(root_path, ClientCapabilities::default());
        let uri = lsp_types::Url::parse("file:///semantic.sv").unwrap();

        let accepted_request_id = lsp_server::RequestId::from(1);
        let accepted_request =
            Request::new(accepted_request_id.clone(), "test/request".to_owned(), ());
        state.register_request(Instant::now(), &accepted_request);
        let accepted_tokens =
            lsp_types::SemanticTokens { result_id: Some("accepted".to_owned()), data: Vec::new() };

        state.process_task(Task::Response(
            ResponseTask::new(Response::new_ok(accepted_request_id.clone(), ()))
                .with_accepted_effects(vec![AcceptedResponseEffect::CommitSemanticTokens {
                    uri: uri.clone(),
                    tokens: accepted_tokens,
                }]),
        ));

        let result_id = state
            .semantic_tokens_cache
            .lock()
            .get(&uri)
            .and_then(|tokens| tokens.result_id.clone());
        assert_eq!(result_id.as_deref(), Some("accepted"));

        let cancelled_request_id = lsp_server::RequestId::from(2);
        let cancelled_request =
            Request::new(cancelled_request_id.clone(), "test/request".to_owned(), ());
        state.register_request(Instant::now(), &cancelled_request);
        state.cancel(cancelled_request_id.clone());
        let cancelled_tokens =
            lsp_types::SemanticTokens { result_id: Some("cancelled".to_owned()), data: Vec::new() };

        state.process_task(Task::Response(
            ResponseTask::new(Response::new_ok(cancelled_request_id, ())).with_accepted_effects(
                vec![AcceptedResponseEffect::CommitSemanticTokens {
                    uri: uri.clone(),
                    tokens: cancelled_tokens,
                }],
            ),
        ));

        let result_id = state
            .semantic_tokens_cache
            .lock()
            .get(&uri)
            .and_then(|tokens| tokens.result_id.clone());
        assert_eq!(result_id.as_deref(), Some("accepted"));
    }

    #[test]
    fn stale_progress_does_not_update_readiness_or_report() {
        let root = TestDir::new("stale-vfs-progress");
        let root_path = root.path().to_path_buf();
        let (mut state, client) = test_state_with_caps(
            root_path,
            ClientCapabilities {
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..WindowClientCapabilities::default()
                }),
                ..ClientCapabilities::default()
            },
        );
        let stale_config = state.workspace_vfs.begin_vfs_load(4);
        let current_config = state.workspace_vfs.begin_vfs_load(4);

        state.process_vfs_msg(vfs_loader::Message::Progress {
            n_total: 4,
            n_done: 4,
            config_version: stale_config,
        });

        assert_eq!(
            state.workspace_vfs.current_vfs_progress(),
            crate::global_state::VfsProgress {
                config_version: current_config,
                n_done: 0,
                n_total: 4,
            }
        );
        assert!(!state.workspace_vfs.is_ready());
        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());

        state.process_vfs_msg(vfs_loader::Message::Progress {
            n_total: 4,
            n_done: 2,
            config_version: current_config,
        });

        assert_eq!(
            state.workspace_vfs.current_vfs_progress(),
            crate::global_state::VfsProgress {
                config_version: current_config,
                n_done: 2,
                n_total: 4,
            }
        );
        let Message::Notification(notification) =
            client.receiver.recv_timeout(Duration::from_secs(1)).unwrap()
        else {
            panic!("expected progress notification");
        };
        assert_eq!(notification.method, lsp_types::notification::Progress::METHOD);
        let params: ProgressParams = serde_json::from_value(notification.params).unwrap();
        let ProgressParamsValue::WorkDone(WorkDoneProgress::Report(report)) = params.value else {
            panic!("expected VFS progress report");
        };
        assert_eq!(report.message.as_deref(), Some("2/4"));
        assert_eq!(report.percentage, Some(50));
    }

    #[test]
    fn out_of_order_vfs_progress_does_not_regress_readiness_or_report() {
        let root = TestDir::new("out-of-order-vfs-progress");
        let root_path = root.path().to_path_buf();
        let (mut state, client) = test_state_with_caps(
            root_path,
            ClientCapabilities {
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..WindowClientCapabilities::default()
                }),
                ..ClientCapabilities::default()
            },
        );
        let config_version = state.workspace_vfs.begin_vfs_load(2);

        state.process_vfs_msg(vfs_loader::Message::Progress {
            n_total: 2,
            n_done: 2,
            config_version,
        });

        assert!(state.workspace_vfs.is_ready());
        assert_eq!(
            state.workspace_vfs.current_vfs_progress(),
            crate::global_state::VfsProgress { config_version, n_done: 2, n_total: 2 }
        );
        assert!(matches!(recv_work_done_progress(&client), WorkDoneProgress::End(_)));

        state.process_vfs_msg(vfs_loader::Message::Progress {
            n_total: 2,
            n_done: 1,
            config_version,
        });

        assert!(state.workspace_vfs.is_ready());
        assert_eq!(
            state.workspace_vfs.current_vfs_progress(),
            crate::global_state::VfsProgress { config_version, n_done: 2, n_total: 2 }
        );
        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());
    }

    #[test]
    fn superseded_workspace_fetch_does_not_commit_stale_workspaces() {
        let root = TestDir::new("superseded-workspace-fetch");
        let root_path = root.path().to_path_buf();
        let existing_root = root.join("existing");
        let stale_root = root.join("stale");
        std::fs::create_dir_all(&existing_root).unwrap();
        std::fs::create_dir_all(&stale_root).unwrap();
        let (mut state, _client) = test_state_with_caps(root_path, ClientCapabilities::default());
        let existing_workspaces = Arc::new(workspace_model(existing_root));
        state.workspaces = Arc::clone(&existing_workspaces);

        state.request_workspace_reload("first reload");
        let first = state.fetch_workspaces_task.should_start().unwrap();
        state.workspace_vfs.start_workspace_fetch(first.generation);
        state.request_workspace_reload("second reload");

        state.process_task(Task::FetchWorkspace(FetchWorkspaceProgress::End {
            generation: first.generation,
            workspaces: workspace_model(stale_root),
            errors: Vec::new(),
        }));

        assert!(Arc::ptr_eq(&state.workspaces, &existing_workspaces));
        assert_eq!(state.workspace_vfs.current_vfs_config_version(), 0);
        let second = state.fetch_workspaces_task.should_start().unwrap();
        assert_eq!(second.cause, "second reload");
        assert_ne!(second.generation, first.generation);
    }

    #[test]
    fn superseded_workspace_fetch_ends_reported_progress() {
        let root = TestDir::new("superseded-workspace-fetch-progress");
        let root_path = root.path().to_path_buf();
        let stale_root = root.join("stale");
        std::fs::create_dir_all(&stale_root).unwrap();
        let (mut state, client) = test_state_with_caps(
            root_path,
            ClientCapabilities {
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..WindowClientCapabilities::default()
                }),
                ..ClientCapabilities::default()
            },
        );

        state.request_workspace_reload("first reload");
        let first = state.fetch_workspaces_task.should_start().unwrap();
        state.workspace_vfs.start_workspace_fetch(first.generation);
        state.process_task(Task::FetchWorkspace(FetchWorkspaceProgress::Begin {
            generation: first.generation,
            cause: first.cause.clone(),
        }));
        assert!(matches!(recv_work_done_progress(&client), WorkDoneProgress::Begin(_)));

        state.request_workspace_reload("second reload");
        state.process_task(Task::FetchWorkspace(FetchWorkspaceProgress::End {
            generation: first.generation,
            workspaces: workspace_model(stale_root),
            errors: Vec::new(),
        }));

        assert!(matches!(recv_work_done_progress(&client), WorkDoneProgress::End(_)));
        assert_eq!(state.workspace_vfs.current_vfs_config_version(), 0);
        let second = state.fetch_workspaces_task.should_start().unwrap();
        assert_eq!(second.cause, "second reload");
        assert_ne!(second.generation, first.generation);
    }
}
impl Task {
    pub(crate) fn response(response: lsp_server::Response) -> Self {
        Task::Response(ResponseTask::new(response))
    }

    pub(in crate::global_state) fn kind(&self) -> &'static str {
        match self {
            Task::Response(_) => "task.response",
            Task::Retry(_) => "task.retry",
            Task::FetchWorkspace(FetchWorkspaceProgress::Begin { .. }) => {
                "task.fetch_workspace.begin"
            }
            Task::FetchWorkspace(FetchWorkspaceProgress::End { .. }) => "task.fetch_workspace.end",
            Task::Diagnostics(_) => "task.diagnostics",
            Task::Qihe(task) => task.kind(),
        }
    }

    pub(in crate::global_state) fn summary(&self) -> String {
        match self {
            Task::Response(response) => response.summary(),
            Task::Retry(req) => format!("task retry method={} id={:?}", req.method, req.id),
            Task::FetchWorkspace(FetchWorkspaceProgress::Begin { cause, .. }) => {
                format!("task fetch workspace begin cause={cause}")
            }
            Task::FetchWorkspace(FetchWorkspaceProgress::End { workspaces, errors, .. }) => {
                format!(
                    "task fetch workspace end workspaces={} errors={}",
                    workspaces.len(),
                    errors.len()
                )
            }
            Task::Diagnostics(tasks) => {
                let diagnostic_count = tasks.diagnostic_count();
                format!(
                    "task diagnostics files={} diagnostics={diagnostic_count}",
                    tasks.touched_file_count()
                )
            }
            Task::Qihe(task) => task.summary(),
        }
    }
}

impl GlobalState {
    pub(in crate::global_state) fn register_did_save_cap(&mut self) {
        let mut document_selector = vec![lsp_types::DocumentFilter {
            language: None,
            scheme: None,
            pattern: Some("**/*.{v,sv,vh,svh,svi}".into()),
        }];
        document_selector.extend(project_manifest::MANIFEST_FILE_NAMES.iter().map(|file_name| {
            lsp_types::DocumentFilter {
                language: None,
                scheme: None,
                pattern: Some(format!("**/{file_name}")),
            }
        }));

        let save_registration_options = lsp_types::TextDocumentSaveRegistrationOptions {
            include_text: false.into(),
            text_document_registration_options: lsp_types::TextDocumentRegistrationOptions {
                document_selector: document_selector.into(),
            },
        };

        let registration = lsp_types::Registration {
            id: "textDocument/didSave".into(),
            method: "textDocument/didSave".into(),
            register_options: match serde_json::to_value(save_registration_options) {
                Ok(options) => Some(options),
                Err(error) => {
                    tracing::error!("failed to serialize didSave registration options: {error:#}");
                    return;
                }
            },
        };
        self.send_request::<lsp_types::request::RegisterCapability>(
            lsp_types::RegistrationParams { registrations: vec![registration] },
            DEFAULT_REQ_HANDLER,
        );
    }

    pub(in crate::global_state) fn handle_request(&mut self, req: Request) {
        if Self::is_pull_diagnostic_request(&req) && !self.is_workspace_ready() {
            self.workspace_vfs.defer_diagnostics_until_ready();
            self.pending_diagnostic_requests.push(req);
            return;
        }

        let mut dispatcher = ReqDispatcher { req: Some(req), global_state: self };

        // Handle shutdown req first
        dispatcher.on_sync_mut::<lsp_types::request::Shutdown>(|this, ()| {
            this.shutdown_requested = true;
            this.cancel_all_tasks();
            Ok(())
        });

        match &mut dispatcher {
            ReqDispatcher { req: Some(req), global_state: this } if this.shutdown_requested => {
                this.respond(lsp_server::Response::new_err(
                    req.id.clone(),
                    lsp_server::ErrorCode::InvalidRequest as i32,
                    this.config.i18n.text(keys::SERVER_SHUTDOWN_ALREADY_REQUESTED).to_owned(),
                ));
                return;
            }
            _ => (),
        }

        use handlers::request::*;
        use lsp_types::request::*;
        dispatcher
            .on_no_retry::<Completion>(handle_completion)
            .on_latency_sensitive::<SemanticTokensFullRequest>(handle_semantic_tokens_full)
            .on_latency_sensitive::<SemanticTokensFullDeltaRequest>(
                handle_semantic_tokens_full_delta,
            )
            .on_latency_sensitive::<SemanticTokensRangeRequest>(handle_semantic_tokens_range)
            .on::<DocumentSymbolRequest>(handle_document_symbol)
            .on::<WorkspaceSymbolRequest>(handle_workspace_symbol)
            .on::<FoldingRangeRequest>(handle_folding_ranges)
            .on::<DocumentDiagnosticRequest>(handle_document_diagnostic)
            .on::<WorkspaceDiagnosticRequest>(handle_workspace_diagnostic)
            .on_no_retry::<SignatureHelpRequest>(handle_signature_help)
            .on_no_retry::<InlayHintRequest>(handle_inlay_hint)
            .on_no_retry::<CodeLensRequest>(handle_code_lens)
            .on_no_retry::<CodeLensResolve>(handle_code_lens_resolve)
            .on_no_retry::<HoverRequest>(handle_hover)
            .on_no_retry::<GotoDefinition>(handle_goto_definition)
            .on_no_retry::<GotoDeclaration>(handle_goto_declaration)
            .on_no_retry::<DocumentHighlightRequest>(handle_document_highlight)
            .on_no_retry::<References>(handle_references)
            .on_no_retry::<PrepareRenameRequest>(handle_prepare_rename)
            .on_no_retry::<Rename>(handle_rename)
            .on_fmt_thread::<Formatting>(handle_formatting)
            .on_fmt_thread::<RangeFormatting>(handle_range_formatting)
            .on_fmt_thread::<OnTypeFormatting>(handle_on_type_formatting)
            .on_no_retry::<CodeActionRequest>(handle_code_action)
            .on_no_retry::<CodeActionResolveRequest>(handle_code_action_resolve)
            .on_sync_mut::<ExecuteCommand>(handle_execute_command)
            .on::<SelectionRangeRequest>(handle_selection_range)
            .finish();
    }

    pub(in crate::global_state) fn handle_notification(&mut self, notif: Notification) {
        use handlers::notification::*;
        use lsp_types::notification::*;

        let mut dispatcher = NotifDispatcher { notif: Some(notif), global_state: self };
        dispatcher
            .on_sync_mut::<Cancel>(handle_cancel)
            .on_sync_mut::<WorkDoneProgressCancel>(handle_work_done_progress_cancel)
            .on_sync_mut::<DidOpenTextDocument>(handle_did_open_text_document)
            .on_sync_mut::<DidChangeTextDocument>(handle_did_change_text_document)
            .on_sync_mut::<DidCloseTextDocument>(handle_did_close_text_document)
            .on_sync_mut::<DidSaveTextDocument>(handle_did_save_text_document)
            .on_sync_mut::<DidChangeConfiguration>(handle_did_change_configuration)
            .on_sync_mut::<DidChangeWorkspaceFolders>(handle_did_change_workspace_folders)
            .on_sync_mut::<DidChangeWatchedFiles>(handle_did_change_watched_files)
            .on_sync_mut::<SetTrace>(handle_set_trace)
            .finish();
    }

    pub(in crate::global_state) fn handle_response(&mut self, res: Response) {
        let Some(handler) = self.req_queue.outgoing.complete(res.id.clone()) else {
            tracing::error!("received response for unknown request: {:?}", res);
            return;
        };
        handler(self, res)
    }

    fn is_pull_diagnostic_request(req: &Request) -> bool {
        matches!(
            req.method.as_str(),
            lsp_types::request::DocumentDiagnosticRequest::METHOD
                | lsp_types::request::WorkspaceDiagnosticRequest::METHOD
        )
    }

    pub(in crate::global_state) fn drain_pending_diagnostic_requests(&mut self) {
        let pending_requests = std::mem::take(&mut self.pending_diagnostic_requests);
        for req in pending_requests {
            if !self.is_completed(&req) {
                self.handle_request(req);
            }
        }
    }
}
