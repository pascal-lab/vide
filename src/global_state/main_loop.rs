use std::time::{Duration, Instant};

use always_assert::always;
use crossbeam_channel::{Receiver, select};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{TraceValue, notification::Notification as _};
use project_model::project_manifest;
use triomphe::Arc;
use utils::thread::ThreadIntent;
use vfs::{FileId, VfsPath, loader as vfs_loader};

use super::{
    GlobalState, VfsProgress,
    dispatcher::{NotifDispatcher, ReqDispatcher},
    handlers,
    qihe::QiheUpdate,
    reload::FetchWorkspaceProgress,
    respond::Progress,
};
use crate::{config::Config, global_state::DEFAULT_REQ_HANDLER, i18n::keys};

#[derive(Debug)]
enum Event {
    Lsp(Message),
    Task(Task),
    Vfs(vfs_loader::Message),
}

#[derive(Debug)]
pub(crate) struct PublishDiagnosticsTask {
    pub(crate) file_id: FileId,
    pub(crate) uri: lsp_types::Url,
    pub(crate) version: Option<i32>,
    pub(crate) diagnostics: Vec<lsp_types::Diagnostic>,
}

#[derive(Debug)]
pub(crate) enum Task {
    Response(lsp_server::Response),
    Retry(lsp_server::Request),
    FetchWorkspace(FetchWorkspaceProgress),
    Diagnostics(Vec<PublishDiagnosticsTask>),
    Qihe(QiheTask),
}

#[derive(Debug)]
pub(crate) enum QiheTask {
    Log { token: String, message: String },
    Finished { update: QiheUpdate, progress_token: String },
    Failed { message: String, progress_token: String },
}

pub fn main_loop(
    config: Config,
    connection: Connection,
    initial_trace: TraceValue,
) -> anyhow::Result<()> {
    tracing::info!("initial config: {:#?}", config);

    // hack for windwos
    #[cfg(windows)]
    unsafe {
        use winapi::um::processthreadsapi::*;
        let thread = GetCurrentThread();
        let thread_priority_above_normal = 1;
        SetThreadPriority(thread, thread_priority_above_normal);
    }

    GlobalState::new(connection.sender, config, initial_trace).run(connection.receiver)
}

impl GlobalState {
    pub(crate) fn run(&mut self, client_receiver: Receiver<Message>) -> anyhow::Result<()> {
        // TODO: check for status

        if self.config.cli_did_save_dyn_reg() {
            self.register_did_save_cap();
        }

        self.fetch_workspaces_task.request("Start".into());
        if let Some(cause) = self.fetch_workspaces_task.should_start() {
            self.fetch_workspaces(cause);
        }

        while let Some(event) = self.next_event(&client_receiver) {
            if let Event::Lsp(Message::Notification(Notification { method, .. })) = &event
                && method == lsp_types::notification::Exit::METHOD
            {
                return Ok(());
            }
            self.handle_event(event)?;
        }
        anyhow::bail!("{} exited without proper shutdown sequence", self.config.opt.process_name);
    }

    fn register_did_save_cap(&mut self) {
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

    fn next_event(&self, cli_inbox: &Receiver<Message>) -> Option<Event> {
        select! {
            recv(cli_inbox) -> cli_msg => cli_msg.ok().map(Event::Lsp),
            recv(self.task_pool.receiver) -> task => task.ok().map(Event::Task),
            recv(self.vfs_loader.receiver) -> vfs_task => vfs_task.ok().map(Event::Vfs),
        }
    }

    fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        let loop_start = Instant::now();

        let event_dbg_msg = format!("{event:?}");
        tracing::debug!("{} [handle_event]: {}", format!("{loop_start:?}"), event_dbg_msg);

        let was_stuck = self.is_stuck();

        match event {
            Event::Lsp(msg) => match msg {
                Message::Request(req) => {
                    self.register_request(loop_start, &req);
                    self.handle_request(req);
                }
                Message::Notification(notif) => self.handle_notification(notif),
                Message::Response(res) => self.handle_response(res),
            },
            Event::Task(task) => self.handle_task(task),
            Event::Vfs(msg) => self.handle_vfs_msg(msg),
        }

        let event_handling_duration = loop_start.elapsed();

        let state_changed = self.process_changes();

        if self.is_stuck() {
            let client_refresh = !was_stuck || state_changed;

            if client_refresh && self.config.cli_code_lens_refresh_support() {
                self.send_request::<lsp_types::request::CodeLensRefresh>((), DEFAULT_REQ_HANDLER);
            }

            if client_refresh && self.config.cli_inlay_hint_refresh_support() {
                self.send_request::<lsp_types::request::InlayHintRefreshRequest>(
                    (),
                    DEFAULT_REQ_HANDLER,
                );
            }
        }

        if self.config.user_config.workspace_auto_reload
            && let Some(cause) = self.fetch_workspaces_task.should_start()
        {
            self.fetch_workspaces(cause);
        }

        let loop_duration = loop_start.elapsed();
        if loop_duration > Duration::from_millis(100) && was_stuck {
            tracing::warn!(
                "overly long loop turn took {loop_duration:?} (event handling took {event_handling_duration:?}): {event_dbg_msg}"
            );
        }

        tracing::debug!("{loop_start:?} [handle_event]: {event_dbg_msg} done in {loop_duration:?}");

        Ok(())
    }

    fn handle_request(&mut self, req: Request) {
        if matches!(
            req.method.as_str(),
            lsp_types::request::DocumentDiagnosticRequest::METHOD
                | lsp_types::request::WorkspaceDiagnosticRequest::METHOD
        ) && !self.is_stuck()
        {
            self.task_pool.handle.spawn_and_send(ThreadIntent::Worker, move || Task::Retry(req));
            return;
        }

        let mut dispatcher = ReqDispatcher { req: Some(req), global_state: self };

        // Handle shutdown req first
        dispatcher.on_sync_mut::<lsp_types::request::Shutdown>(|this, ()| {
            this.shutdown_requested = true;
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
            .on_sync::<OnTypeFormatting>(handle_on_type_formatting)
            .on_no_retry::<CodeActionRequest>(handle_code_action)
            .on_no_retry::<CodeActionResolveRequest>(handle_code_action_resolve)
            .on_sync_mut::<ExecuteCommand>(handle_execute_command)
            .on::<SelectionRangeRequest>(handle_selection_range)
            .finish();
    }

    fn handle_notification(&mut self, notif: Notification) {
        use handlers::notification::*;
        use lsp_types::notification::*;

        let mut dispatcher = NotifDispatcher { notif: Some(notif), global_state: self };
        dispatcher
            .on_sync_mut::<Cancel>(handle_cancel)
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

    fn handle_response(&mut self, res: Response) {
        let Some(handler) = self.req_queue.outgoing.complete(res.id.clone()) else {
            tracing::error!("received response for unknown request: {:?}", res);
            return;
        };
        handler(self, res)
    }

    fn handle_task(&mut self, task: Task) {
        self.process_task(task);

        // Coalesce task events in one turn
        while let Ok(task) = self.task_pool.receiver.try_recv() {
            self.process_task(task);
        }

        // TODO: PrimaryCache?
    }

    fn process_task(&mut self, task: Task) {
        match task {
            Task::Response(res) => self.respond(res),
            Task::Retry(req) => {
                if !self.is_completed(&req) {
                    self.handle_request(req);
                }
            }
            Task::FetchWorkspace(process) => {
                let state = match process {
                    FetchWorkspaceProgress::Begin(cause) => {
                        self.send_loading_project_status(cause);
                        Progress::Begin
                    }
                    FetchWorkspaceProgress::End(workspaces, errors) => {
                        let workspace_count = workspaces.len();
                        let error_messages =
                            errors.iter().map(|err| format!("{err:#}")).collect::<Vec<_>>();

                        self.fetch_workspaces_task.complete(Some((Arc::new(workspaces), errors)));

                        if let Err(e) = self.fetch_workspace_error_stringify() {
                            tracing::error!("Fetch workspace error: \n{e}");
                        }

                        self.switch_workspaces("fetched new workspaces".into());
                        self.send_project_status_for_result(workspace_count, &error_messages);

                        Progress::End
                    }
                };

                self.report_progress(
                    self.config.i18n.text(keys::PROGRESS_FETCHING_WORKSPACES),
                    state,
                    None,
                    None,
                    None,
                );
            }
            Task::Diagnostics(diags) => self.publish_diagnostics_tasks(diags, false),
            Task::Qihe(task) => self.handle_qihe_task(task),
        }
    }

    fn handle_vfs_msg(&mut self, msg: vfs_loader::Message) {
        self.process_vfs_msg(msg);

        // Coalesce task events in one turn
        while let Ok(msg) = self.vfs_loader.receiver.try_recv() {
            self.process_vfs_msg(msg);
        }
    }

    fn process_vfs_msg(&mut self, msg: vfs_loader::Message) {
        match msg {
            vfs_loader::Message::Progress { n_total, n_done, config_version } => {
                always!(config_version <= self.vfs_config_version);

                self.vfs_progress = VfsProgress { config_version, n_done, n_total };

                let state = if n_done == 0 {
                    Progress::Begin
                } else if n_done < n_total {
                    Progress::Report
                } else {
                    assert_eq!(n_done, n_total);
                    Progress::End
                };

                self.report_progress(
                    self.config.i18n.text(keys::PROGRESS_ROOTS_SCANNING),
                    state,
                    Some(format!("{n_done}/{n_total}")),
                    Some(Progress::fraction(n_done, n_total)),
                    None,
                );
            }
            vfs_loader::Message::Loaded { files } => {
                let vfs = &mut self.vfs.write().0;

                for (path, content) in files {
                    let path = VfsPath::from(path);
                    if !self.mem_docs.contains_path(&path) {
                        vfs.set_file_contents(&path, content);
                    }
                }
            }
        }
    }

    pub(super) fn publish_diagnostics_tasks(
        &mut self,
        diagnostics: Vec<PublishDiagnosticsTask>,
        force_push: bool,
    ) {
        if self.config.cli_pull_diagnostics_support() && !force_push {
            return;
        }

        for diag in diagnostics {
            let should_publish = match self.diagnostics.get(&diag.file_id) {
                Some(prev) => prev != &diag.diagnostics,
                None => !diag.diagnostics.is_empty(),
            };

            if !should_publish {
                continue;
            }

            if diag.diagnostics.is_empty() {
                self.diagnostics.remove(&diag.file_id);
            } else {
                self.diagnostics.insert(diag.file_id, diag.diagnostics.clone());
            }

            self.send_notification::<lsp_types::notification::PublishDiagnostics>(
                lsp_types::PublishDiagnosticsParams {
                    uri: diag.uri,
                    diagnostics: diag.diagnostics,
                    version: diag.version,
                },
            );
        }
    }
}
