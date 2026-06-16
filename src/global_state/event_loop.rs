use std::time::{Duration, Instant};

use always_assert::always;
use crossbeam_channel::{Receiver, select};
use lsp_server::{Connection, Message, Notification};
use lsp_types::{TraceValue, notification::Notification as _};
use triomphe::Arc;
use vfs::{VfsPath, loader as vfs_loader};

use super::{
    GlobalState, WorkspaceFetchCompletion,
    process_changes::DiagnosticInvalidation,
    reload::FetchWorkspaceProgress,
    respond::Progress,
    task::{ResponseTask, Task},
};
use crate::{config::Config, global_state::DEFAULT_REQ_HANDLER, i18n::keys};

#[derive(Debug)]
pub(in crate::global_state) enum Event {
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
                self.cancel_all_tasks();
                return Ok(());
            }
            self.handle_event(event)?;
        }
        anyhow::bail!("{} exited without proper shutdown sequence", self.config.opt.process_name);
    }

    pub(crate) fn handle_lsp_message_for_browser(&mut self, msg: Message) -> anyhow::Result<()> {
        self.handle_event(Event::Lsp(msg))?;
        self.drain_browser_queued_events()
    }

    pub(crate) fn drain_browser_queued_events(&mut self) -> anyhow::Result<()> {
        while let Ok(task) = self.task_pool.receiver.try_recv() {
            self.handle_event(Event::Task(task))?;
        }

        while let Ok(msg) = self.vfs_loader.receiver.try_recv() {
            self.handle_event(Event::Vfs(msg))?;
        }

        while let Ok(task) = self.task_pool.receiver.try_recv() {
            self.handle_event(Event::Task(task))?;
        }
        Ok(())
    }

    fn next_event(&self, cli_inbox: &Receiver<Message>) -> Option<Event> {
        select! {
            recv(cli_inbox) -> cli_msg => cli_msg.ok().map(Event::Lsp),
            recv(self.task_pool.receiver) -> task => task.ok().map(Event::Task),
            recv(self.vfs_loader.receiver) -> vfs_task => vfs_task.ok().map(Event::Vfs),
        }
    }

    pub(in crate::global_state) fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
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

        let was_workspace_ready = self.is_workspace_ready();
        let event_span = tracing::info_span!(
            "main_loop.handle_event",
            event.kind = event_kind,
            event.summary = %event_summary,
            was_workspace_ready
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
        if self.workspace_vfs.take_deferred_diagnostics_if_ready() {
            self.invalidate_diagnostics(DiagnosticInvalidation::WorkspaceChanged);
            self.drain_pending_diagnostic_requests();
        }

        if self.is_workspace_ready() {
            let client_refresh = !was_workspace_ready || state_changed;

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
        if loop_duration > Duration::from_millis(100) && was_workspace_ready {
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

    fn handle_task(&mut self, task: Task) {
        self.process_task(task);

        // Coalesce task events in one turn
        while let Ok(task) = self.task_pool.receiver.try_recv() {
            self.process_task(task);
        }

        // TODO: PrimaryCache?
    }

    pub(in crate::global_state) fn process_task(&mut self, task: Task) {
        let task_kind = task.kind();
        let task_summary = task.summary();
        let _span = tracing::info_span!(
            "main_loop.process_task",
            task.kind = task_kind,
            task.summary = %task_summary
        )
        .entered();

        match task {
            Task::Response(response) => self.respond_task(response),
            Task::Retry(req) => {
                if !self.is_completed(&req) {
                    self.handle_request(req);
                }
            }
            Task::FetchWorkspace(process) => {
                let Some(state) = (match process {
                    FetchWorkspaceProgress::Begin { generation, cause } => {
                        if !self.workspace_vfs.accept_workspace_fetch_begin(generation) {
                            tracing::debug!(?generation, "stale workspace fetch begin ignored");
                            return;
                        }
                        self.send_loading_project_status(cause);
                        Some(Progress::Begin)
                    }
                    FetchWorkspaceProgress::End { generation, workspaces, errors } => {
                        let workspace_count = workspaces.len();
                        let error_messages =
                            errors.iter().map(|err| format!("{err:#}")).collect::<Vec<_>>();
                        let completion = self
                            .workspace_vfs
                            .finish_workspace_fetch(generation, !errors.is_empty());

                        match completion {
                            WorkspaceFetchCompletion::Stale { progress_started } => {
                                self.fetch_workspaces_task.complete(None);
                                tracing::debug!(
                                    ?generation,
                                    "stale workspace fetch result ignored"
                                );
                                progress_started.then_some(Progress::End)
                            }
                            WorkspaceFetchCompletion::CurrentFailure => {
                                self.fetch_workspaces_task
                                    .complete(Some((Arc::new(workspaces), errors)));
                                if let Err(e) = self.fetch_workspace_error_stringify() {
                                    tracing::error!("Fetch workspace error: \n{e}");
                                }
                                self.send_project_status_for_result(
                                    workspace_count,
                                    &error_messages,
                                );
                                Some(Progress::End)
                            }
                            WorkspaceFetchCompletion::CurrentSuccess => {
                                self.fetch_workspaces_task
                                    .complete(Some((Arc::new(workspaces), errors)));

                                self.switch_workspaces("fetched new workspaces".into(), generation);
                                self.send_project_status_for_result(
                                    workspace_count,
                                    &error_messages,
                                );

                                Some(Progress::End)
                            }
                        }
                    }
                }) else {
                    return;
                };

                self.report_progress(
                    self.config.i18n.text(keys::PROGRESS_FETCHING_WORKSPACES),
                    state,
                    None,
                    None,
                    None,
                );
            }
            Task::Diagnostics(diags) => self.publish_diagnostics_tasks(diags),
            Task::Qihe(task) => self.handle_qihe_task(task),
        }
    }

    fn respond_task(&mut self, task: ResponseTask) {
        if self.respond(task.response) {
            for effect in task.accepted_effects {
                effect.apply(self);
            }
        }
    }

    fn handle_vfs_msg(&mut self, msg: vfs_loader::Message) {
        self.process_vfs_msg(msg);

        // Coalesce task events in one turn
        while let Ok(msg) = self.vfs_loader.receiver.try_recv() {
            self.process_vfs_msg(msg);
        }
    }

    pub(in crate::global_state) fn process_vfs_msg(&mut self, msg: vfs_loader::Message) {
        match msg {
            vfs_loader::Message::Progress { n_total, n_done, config_version } => {
                always!(config_version <= self.workspace_vfs.current_vfs_config_version());

                let Some(progress) =
                    self.workspace_vfs.accept_vfs_progress(config_version, n_done, n_total)
                else {
                    tracing::debug!(
                        config_version,
                        current_config_version = self.workspace_vfs.current_vfs_config_version(),
                        "stale VFS progress ignored"
                    );
                    return;
                };

                if progress.n_total == 0 {
                    return;
                }

                let state = if progress.n_done == 0 {
                    Progress::Begin
                } else if progress.n_done < progress.n_total {
                    Progress::Report
                } else {
                    assert_eq!(progress.n_done, progress.n_total);
                    Progress::End
                };

                self.report_progress(
                    self.config.i18n.text(keys::PROGRESS_ROOTS_SCANNING),
                    state,
                    Some(format!("{}/{}", progress.n_done, progress.n_total)),
                    Some(Progress::fraction(progress.n_done, progress.n_total)),
                    None,
                );
            }
            vfs_loader::Message::Loaded { files, config_version } => {
                always!(config_version <= self.workspace_vfs.current_vfs_config_version());
                if !self.workspace_vfs.accepts_vfs_loaded(config_version) {
                    tracing::debug!(
                        config_version,
                        current_config_version = self.workspace_vfs.current_vfs_config_version(),
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
}
