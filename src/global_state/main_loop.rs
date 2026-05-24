use std::time::{Duration, Instant};

use always_assert::always;
use crossbeam_channel::{Receiver, select};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::{TraceValue, notification::Notification as _};
use project_model::project_manifest;
use rustc_hash::FxHashSet;
use triomphe::Arc;
use utils::thread::ThreadIntent;
use vfs::{FileId, VfsPath, loader as vfs_loader};

use super::{
    GlobalState, VfsProgress,
    dispatcher::{NotifDispatcher, ReqDispatcher},
    handlers,
    qihe::{QiheRunId, QiheUpdate},
    reload::FetchWorkspaceProgress,
    respond::Progress,
    snapshot::DiagnosticPublishTarget,
};
use crate::{config::Config, global_state::DEFAULT_REQ_HANDLER, i18n::keys};

#[derive(Debug)]
enum Event {
    Lsp(Message),
    Task(Task),
    Vfs(vfs_loader::Message),
}

impl Event {
    fn kind(&self) -> &'static str {
        match self {
            Event::Lsp(Message::Request(_)) => "lsp.request",
            Event::Lsp(Message::Notification(_)) => "lsp.notification",
            Event::Lsp(Message::Response(_)) => "lsp.response",
            Event::Task(task) => task.kind(),
            Event::Vfs(vfs_loader::Message::Progress { .. }) => "vfs.progress",
            Event::Vfs(vfs_loader::Message::Loaded { .. }) => "vfs.loaded",
        }
    }

    fn summary(&self) -> String {
        match self {
            Event::Lsp(Message::Request(req)) => {
                format!("request method={} id={:?}", req.method, req.id)
            }
            Event::Lsp(Message::Notification(notif)) => {
                format!("notification method={}", notif.method)
            }
            Event::Lsp(Message::Response(res)) => {
                format!("response id={:?} error={}", res.id, res.error.is_some())
            }
            Event::Task(task) => task.summary(),
            Event::Vfs(vfs_loader::Message::Progress { n_done, n_total, .. }) => {
                format!("vfs progress {n_done}/{n_total}")
            }
            Event::Vfs(vfs_loader::Message::Loaded { files, .. }) => {
                format!("vfs loaded files={}", files.len())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use lsp_server::Connection;
    use lsp_types::{
        Diagnostic, DiagnosticSeverity, Position, PublishDiagnosticsParams, Range,
        notification::Notification as _,
    };
    use utils::{lines::LineEnding, paths::AbsPathBuf, test_support::TestDir};
    use vfs::loader::LoadResult;

    use super::*;
    use crate::{Opt, config::user_config::UserConfig, i18n::I18n, lsp_ext::to_proto};

    fn test_state(root_path: AbsPathBuf) -> GlobalState {
        let config = Config::new(
            Opt {
                process_name: "vizsla-test".to_string(),
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

        let (server, _client) = Connection::memory();
        GlobalState::new(server.sender, config, lsp_types::TraceValue::Off)
    }

    fn recv_publish(client: &Connection) -> PublishDiagnosticsParams {
        let message = client.receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let lsp_server::Message::Notification(notification) = message else {
            panic!("expected publishDiagnostics notification");
        };
        assert_eq!(notification.method, lsp_types::notification::PublishDiagnostics::METHOD);
        serde_json::from_value(notification.params).unwrap()
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
                process_name: "vizsla-test".to_string(),
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

        state.publish_diagnostics_tasks(
            publish_batch(vec![PublishDiagnosticsTask::for_test(
                file_id,
                primary_uri.clone(),
                None,
                vec![diagnostic.clone()],
            )]),
            false,
        );
        let first = recv_publish(&client);
        assert_eq!(first.uri, primary_uri);
        assert_eq!(first.diagnostics, vec![diagnostic.clone()]);

        state.publish_diagnostics_tasks(
            publish_batch(vec![PublishDiagnosticsTask::for_test(
                file_id,
                alias_uri.clone(),
                Some(7),
                vec![diagnostic.clone()],
            )]),
            false,
        );

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
                process_name: "vizsla-test".to_string(),
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

        state.publish_diagnostics_tasks(
            publish_batch(vec![PublishDiagnosticsTask::for_test(
                file_id,
                alias_uri.clone(),
                Some(9),
                vec![diagnostic],
            )]),
            false,
        );
        let published = recv_publish(&client);
        assert_eq!(published.uri, alias_uri);
        assert!(!published.diagnostics.is_empty());

        state.publish_diagnostics_tasks(
            PublishDiagnosticsBatch::for_touched_files(
                FxHashSet::from_iter([file_id]),
                Vec::new(),
                DiagnosticPublishFreshness::default(),
            ),
            false,
        );

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
                process_name: "vizsla-test".to_string(),
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

        state.publish_diagnostics_tasks(
            PublishDiagnosticsBatch::from_tasks(
                vec![PublishDiagnosticsTask::for_test(file_id, uri, None, vec![diagnostic])],
                DiagnosticPublishFreshness::new(1, 0),
            ),
            false,
        );

        assert!(client.receiver.recv_timeout(Duration::from_millis(50)).is_err());
        assert!(state.published_diagnostics.is_empty());
    }

    #[test]
    fn stale_loaded_batches_do_not_update_vfs() {
        let root = TestDir::new("stale-loaded-batches");
        let root_path = root.path().to_path_buf();
        let file_path = root_path.join("stale.sv");
        let mut state = test_state(root_path);
        state.vfs_config_version = 2;

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
}

#[derive(Debug)]
pub(crate) struct PublishDiagnosticsTask {
    file_id: FileId,
    uri: lsp_types::Url,
    version: Option<i32>,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

#[derive(Debug)]
pub(crate) struct PublishDiagnosticsBatch {
    freshness: DiagnosticPublishFreshness,
    touched_file_ids: FxHashSet<FileId>,
    tasks: Vec<PublishDiagnosticsTask>,
}

/// Freshness token for a diagnostics publish batch.
///
/// Diagnostic contents and diagnostic publish targets can change
/// independently. VFS/content/config changes advance `diagnostics_revision`;
/// didOpen/didClose and identity remaps advance `target_revision` because they
/// change which URIs are live publish targets without necessarily changing the
/// analysis text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DiagnosticPublishFreshness {
    diagnostics_revision: u64,
    target_revision: u64,
}

impl DiagnosticPublishFreshness {
    pub(crate) fn new(diagnostics_revision: u64, target_revision: u64) -> Self {
        Self { diagnostics_revision, target_revision }
    }
}

impl PublishDiagnosticsBatch {
    pub(crate) fn from_tasks(
        tasks: Vec<PublishDiagnosticsTask>,
        freshness: DiagnosticPublishFreshness,
    ) -> Self {
        let touched_file_ids = tasks.iter().map(|task| task.file_id).collect();
        Self { freshness, touched_file_ids, tasks }
    }

    /// Builds a diagnostics batch for files whose publish target set may have
    /// changed independently from diagnostics contents.
    ///
    /// This is what lets didClose clear stale URI diagnostics even when the
    /// remaining target set is empty.
    pub(crate) fn for_touched_files(
        touched_file_ids: FxHashSet<FileId>,
        tasks: Vec<PublishDiagnosticsTask>,
        freshness: DiagnosticPublishFreshness,
    ) -> Self {
        Self { freshness, touched_file_ids, tasks }
    }

    #[cfg(test)]
    pub(crate) fn touched_file_ids(&self) -> &FxHashSet<FileId> {
        &self.touched_file_ids
    }

    #[cfg(test)]
    pub(crate) fn tasks(&self) -> &[PublishDiagnosticsTask] {
        &self.tasks
    }

    #[cfg(test)]
    pub(crate) fn into_tasks(self) -> Vec<PublishDiagnosticsTask> {
        self.tasks
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DiagnosticPublishKey {
    /// Diagnostics are computed per analysis file but delivered per LSP URI.
    /// Keeping both parts in the cache key prevents an alias URI from being
    /// skipped just because the same diagnostic list was sent to another URI.
    file_id: FileId,
    uri: lsp_types::Url,
}

impl DiagnosticPublishKey {
    fn new(file_id: FileId, uri: lsp_types::Url) -> Self {
        Self { file_id, uri }
    }

    #[cfg(test)]
    pub(crate) fn for_test(file_id: FileId, uri: lsp_types::Url) -> Self {
        Self::new(file_id, uri)
    }
}

impl PublishDiagnosticsTask {
    pub(crate) fn from_target(
        target: DiagnosticPublishTarget,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Self {
        let (file_id, uri, version) = target.into_parts();
        Self { file_id, uri, version, diagnostics }
    }

    /// Clears diagnostics previously published to a concrete URI that is no
    /// longer a live target, such as a deleted duplicate identity spelling.
    ///
    /// Normal diagnostics publishing should use [`Self::from_target`] so URI
    /// and version always come from the same document target.
    pub(crate) fn clear_stale_uri(file_id: FileId, uri: lsp_types::Url) -> Self {
        Self { file_id, uri, version: None, diagnostics: Vec::new() }
    }

    #[cfg(test)]
    pub(crate) fn file_id(&self) -> FileId {
        self.file_id
    }

    #[cfg(test)]
    pub(crate) fn uri(&self) -> &lsp_types::Url {
        &self.uri
    }

    #[cfg(test)]
    pub(crate) fn version(&self) -> Option<i32> {
        self.version
    }

    fn cache_key(&self) -> DiagnosticPublishKey {
        DiagnosticPublishKey::new(self.file_id, self.uri.clone())
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        file_id: FileId,
        uri: lsp_types::Url,
        version: Option<i32>,
        diagnostics: Vec<lsp_types::Diagnostic>,
    ) -> Self {
        Self { file_id, uri, version, diagnostics }
    }
}

#[derive(Debug)]
pub(crate) enum Task {
    Response(lsp_server::Response),
    Retry(lsp_server::Request),
    FetchWorkspace(FetchWorkspaceProgress),
    Diagnostics(PublishDiagnosticsBatch),
    Qihe(QiheTask),
}

#[derive(Debug)]
pub(crate) enum QiheTask {
    Log { run_id: QiheRunId, token: String, message: String },
    Finished { run_id: QiheRunId, update: QiheUpdate, progress_token: String },
    Failed { run_id: QiheRunId, message: String, progress_token: String },
}

impl Task {
    fn kind(&self) -> &'static str {
        match self {
            Task::Response(_) => "task.response",
            Task::Retry(_) => "task.retry",
            Task::FetchWorkspace(FetchWorkspaceProgress::Begin(_)) => "task.fetch_workspace.begin",
            Task::FetchWorkspace(FetchWorkspaceProgress::End(_, _)) => "task.fetch_workspace.end",
            Task::Diagnostics(_) => "task.diagnostics",
            Task::Qihe(task) => task.kind(),
        }
    }

    fn summary(&self) -> String {
        match self {
            Task::Response(res) => {
                format!("task response id={:?} error={}", res.id, res.error.is_some())
            }
            Task::Retry(req) => format!("task retry method={} id={:?}", req.method, req.id),
            Task::FetchWorkspace(FetchWorkspaceProgress::Begin(cause)) => {
                format!("task fetch workspace begin cause={cause}")
            }
            Task::FetchWorkspace(FetchWorkspaceProgress::End(workspaces, errors)) => {
                format!(
                    "task fetch workspace end workspaces={} errors={}",
                    workspaces.len(),
                    errors.len()
                )
            }
            Task::Diagnostics(tasks) => {
                let diagnostic_count = diagnostic_task_item_count(&tasks.tasks);
                format!(
                    "task diagnostics files={} diagnostics={diagnostic_count}",
                    tasks.touched_file_ids.len()
                )
            }
            Task::Qihe(task) => task.summary(),
        }
    }
}

impl QiheTask {
    fn kind(&self) -> &'static str {
        match self {
            QiheTask::Log { .. } => "task.qihe.log",
            QiheTask::Finished { .. } => "task.qihe.finished",
            QiheTask::Failed { .. } => "task.qihe.failed",
        }
    }

    fn summary(&self) -> String {
        match self {
            QiheTask::Log { token, message, .. } => {
                format!("task qihe log token={token} bytes={}", message.len())
            }
            QiheTask::Finished { progress_token, .. } => {
                format!("task qihe finished token={progress_token}")
            }
            QiheTask::Failed { progress_token, message, .. } => {
                format!("task qihe failed token={progress_token} message={message}")
            }
        }
    }
}

fn diagnostic_task_item_count(tasks: &[PublishDiagnosticsTask]) -> usize {
    tasks.iter().map(|task| task.diagnostics.len()).sum()
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

        self.request_workspace_reload("Start");
        self.start_requested_workspace_fetch();

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
        let event_kind = event.kind();
        let event_summary = event.summary();

        let event_dbg_msg = {
            let _span = tracing::info_span!(
                "main_loop.event_debug_format",
                event.kind = event_kind,
                event.summary = %event_summary
            )
            .entered();
            format!("{event:?}")
        };
        tracing::debug!(event.summary = %event_summary, "handle_event start");

        let was_stuck = self.is_stuck();
        let event_span = tracing::info_span!(
            "main_loop.handle_event",
            event.kind = event_kind,
            event.summary = %event_summary,
            was_stuck
        );
        let _event_span = event_span.enter();

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

        self.start_requested_workspace_fetch();

        let loop_duration = loop_start.elapsed();
        if loop_duration > Duration::from_millis(100) && was_stuck {
            tracing::warn!(
                event.summary = %event_summary,
                event.debug_len = event_dbg_msg.len(),
                ?loop_duration,
                ?event_handling_duration,
                "overly long loop turn"
            );
        }

        tracing::debug!(
            event.summary = %event_summary,
            event.debug_len = event_dbg_msg.len(),
            ?loop_duration,
            "handle_event done"
        );

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
        let task_kind = task.kind();
        let task_summary = task.summary();
        let _span = tracing::info_span!(
            "main_loop.process_task",
            task.kind = task_kind,
            task.summary = %task_summary
        )
        .entered();

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
            vfs_loader::Message::Loaded { files, config_version } => {
                always!(config_version <= self.vfs_config_version);
                if config_version < self.vfs_config_version {
                    tracing::debug!(
                        config_version,
                        current_config_version = self.vfs_config_version,
                        files = files.len(),
                        "stale VFS loaded batch ignored"
                    );
                    return;
                }

                let vfs = &mut self.vfs.write().0;

                for (path, content) in files {
                    let path = VfsPath::from(path);
                    let open_file_id = vfs
                        .file_id(&path)
                        .is_some_and(|file_id| self.mem_docs.contains_file_id(file_id));
                    if !self.mem_docs.contains_path(&path) && !open_file_id {
                        vfs.set_file_contents(&path, content);
                    }
                }
            }
        }
    }

    pub(super) fn publish_diagnostics_tasks(
        &mut self,
        batch: PublishDiagnosticsBatch,
        force_push: bool,
    ) {
        let task_count = batch.touched_file_ids.len();
        let diagnostic_count = diagnostic_task_item_count(&batch.tasks);
        let _span =
            tracing::info_span!("diagnostics.publish", task_count, diagnostic_count, force_push)
                .entered();

        if self.config.cli_pull_diagnostics_support() && !force_push {
            tracing::info!("skipping push diagnostics for pull-capable client");
            return;
        }

        let mut published_files = 0usize;
        let mut published_diagnostics = 0usize;
        let mut skipped_files = 0usize;
        let PublishDiagnosticsBatch { freshness, touched_file_ids, tasks } = batch;
        let current_freshness = self.diagnostic_publish_freshness();
        if freshness != current_freshness {
            tracing::debug!(?freshness, ?current_freshness, "stale diagnostics batch ignored");
            return;
        }
        let current_targets =
            tasks.iter().map(PublishDiagnosticsTask::cache_key).collect::<FxHashSet<_>>();
        let stale_targets = self
            .published_diagnostics
            .keys()
            .filter(|key| touched_file_ids.contains(&key.file_id) && !current_targets.contains(key))
            .cloned()
            .collect::<Vec<_>>();
        for key in stale_targets {
            self.published_diagnostics.remove(&key);
            self.send_notification::<lsp_types::notification::PublishDiagnostics>(
                lsp_types::PublishDiagnosticsParams {
                    uri: key.uri,
                    diagnostics: Vec::new(),
                    version: None,
                },
            );
            published_files += 1;
        }

        for diag in tasks {
            let file_diagnostics = diag.diagnostics.len();
            let cache_key = diag.cache_key();
            let should_publish = match self.published_diagnostics.get(&cache_key) {
                Some(prev) => prev != &diag.diagnostics,
                None => !diag.diagnostics.is_empty(),
            };

            if !should_publish {
                skipped_files += 1;
                continue;
            }

            if diag.diagnostics.is_empty() {
                self.published_diagnostics.remove(&cache_key);
            } else {
                self.published_diagnostics.insert(cache_key, diag.diagnostics.clone());
            }

            self.send_notification::<lsp_types::notification::PublishDiagnostics>(
                lsp_types::PublishDiagnosticsParams {
                    uri: diag.uri,
                    diagnostics: diag.diagnostics,
                    version: diag.version,
                },
            );
            published_files += 1;
            published_diagnostics += file_diagnostics;
        }
        tracing::info!(
            published_files,
            published_diagnostics,
            skipped_files,
            "publish diagnostics complete"
        );
    }
}
