use std::time::{Duration, Instant};

use always_assert::always;
use const_format::formatcp;
use crossbeam_channel::{select, Receiver};
use lsp_server::{Connection, Notification, Request, Response};
use lsp_types::notification::Notification as _;
use project_model::project_manifest;
use triomphe::Arc;
use vfs::vfs::VfsPath;

use crate::{
    dispatcher::{NotifDispatcher, ReqDispatcher},
    global_state::GlobalState,
    reload::FetchWorkspaceProgress,
    respond::Progress,
    Config,
};

#[derive(Debug)]
enum Event {
    Lsp(lsp_server::Message),
    Task(Task),
    Vfs(vfs::loader::Message),
}

#[derive(Debug)]
pub(crate) enum Task {
    Response(lsp_server::Response),
    Retry(lsp_server::Request),
    FetchWorkspace(FetchWorkspaceProgress),
    // Diagnostics(Vec<(FileId, Vec<lsp_types::Diagnostic>)>)
}

pub fn main_loop(config: Config, connection: Connection) -> anyhow::Result<()> {
    tracing::info!("initial config: {:#?}", config);

    // hack for windwos
    #[cfg(windows)]
    unsafe {
        use winapi::um::processthreadsapi::*;
        let thread = GetCurrentThread();
        let thread_priority_above_normal = 1;
        SetThreadPriority(thread, thread_priority_above_normal);
    }

    GlobalState::new(connection.sender, config).run(connection.receiver)
}

impl GlobalState {
    pub(crate) fn run(&mut self, cli_inbox: Receiver<lsp_server::Message>) -> anyhow::Result<()> {
        // TODO: check for status

        if self.config.cli_did_save_dyn_reg() {
            self.register_did_save_cap();
        }

        self.fetch_workspace_task.request("Start".into(), ());
        if let Some((cause, ())) = self.fetch_workspace_task.can_start() {
            self.fetch_workspaces(cause);
        }

        while let Some(event) = self.next_event(&cli_inbox) {
            match &event {
                Event::Lsp(lsp_server::Message::Notification(Notification { method, .. }))
                    if method == lsp_types::notification::Exit::METHOD =>
                {
                    return Ok(());
                }
                _ => self.handle_event(event)?,
            }
        }
        anyhow::bail!("{} exited without proper shutdown sequence", &self.config.opt.process_name);
    }

    fn register_did_save_cap(&mut self) {
        let save_registration_options = lsp_types::TextDocumentSaveRegistrationOptions {
            include_text: false.into(),
            text_document_registration_options: lsp_types::TextDocumentRegistrationOptions {
                document_selector: vec![
                    lsp_types::DocumentFilter {
                        language: None,
                        scheme: None,
                        pattern: Some("**/*.{v,sv}".into()),
                    },
                    lsp_types::DocumentFilter {
                        language: None,
                        scheme: None,
                        pattern: Some(
                            formatcp!("**/{}", project_manifest::MANIFEST_FILE_NAME).into(),
                        ),
                    },
                ]
                .into(),
            },
        };

        let registration = lsp_types::Registration {
            id: "textDocument/didSave".to_string(),
            method: "textDocument/didSave".to_string(),
            register_options: Some(serde_json::to_value(save_registration_options).unwrap()),
        };
        self.send_request::<lsp_types::request::RegisterCapability>(
            lsp_types::RegistrationParams { registrations: vec![registration] },
            |_, _| (),
        );
    }

    fn next_event(&self, cli_inbox: &Receiver<lsp_server::Message>) -> Option<Event> {
        select! {
            recv(cli_inbox) -> cli_msg => cli_msg.ok().map(Event::Lsp),
            recv(self.task_pool.receiver) -> task => Some(Event::Task(task.unwrap())),
            recv(self.vfs_loader.receiver) -> vfs_task => Some(Event::Vfs(vfs_task.unwrap())),
        }
    }

    fn handle_event(&mut self, event: Event) -> anyhow::Result<()> {
        let loop_start = Instant::now();

        let event_dbg_msg = format!("{event:?}");
        tracing::debug!("{} [handle_event]: {}", format!("{loop_start:?}"), event_dbg_msg);

        // check if is quiescent when loading workspace
        match event {
            Event::Lsp(msg) => match msg {
                lsp_server::Message::Request(req) => self.handle_lsp_request(loop_start, req),
                lsp_server::Message::Notification(notif) => self.handle_lsp_notification(notif)?,
                lsp_server::Message::Response(res) => self.handle_response(res),
            },
            Event::Task(task) => self.handle_task(task),
            Event::Vfs(msg) => self.handle_vfs_msg(msg),
        }

        let event_handling_duration = loop_start.elapsed();

        let loop_duration = loop_start.elapsed();
        if loop_duration > Duration::from_millis(100) {
            tracing::warn!("overly long loop turn took {loop_duration:?} (event handling took {event_handling_duration:?}): {event_dbg_msg}");
        }

        Ok(())
    }

    fn handle_lsp_request(&mut self, req_received: Instant, req: Request) {
        self.register_request(req_received, &req);
        self.dispatch_request(req);
    }

    fn dispatch_request(&mut self, req: Request) {
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
                    "Shutdown already requested.".to_owned(),
                ));
                return;
            }
            _ => (),
        }

        // TODO: Add handlers
        dispatcher.finish();
    }

    fn handle_lsp_notification(&mut self, notif: Notification) -> anyhow::Result<()> {
        NotifDispatcher { notif: Some(notif), global_state: self }.finish();

        Ok(())
    }

    fn handle_response(&mut self, res: Response) {
        let handler = self
            .req_queue
            .outgoing
            .complete(res.id.clone())
            .expect("Received response for unknown request");
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
                    self.dispatch_request(req);
                }
            }
            Task::FetchWorkspace(process) => {
                let state = match process {
                    FetchWorkspaceProgress::Begin => Progress::Begin,
                    FetchWorkspaceProgress::End(workspaces, errors) => {
                        self.fetch_workspace_task.complete(Some((Arc::new(workspaces), errors)));

                        if let Err(e) = self.fetch_workspace_error_stringify() {
                            tracing::error!("Fetch workspace error: \n{e}");
                        }

                        self.switch_workspaces("fetched new workspaces".into());

                        Progress::End
                    }
                };

                self.report_progress("Fetching Workspaces", state, None, None, None);
            }
        }
    }

    fn handle_vfs_msg(&mut self, msg: vfs::loader::Message) {
        self.process_vfs_msg(msg);

        // Coalesce task events in one turn
        while let Ok(msg) = self.vfs_loader.receiver.try_recv() {
            self.process_vfs_msg(msg);
        }
    }

    fn process_vfs_msg(&mut self, msg: vfs::loader::Message) {
        match msg {
            vfs::loader::Message::Progress { n_total, n_done, config_version } => {
                always!(config_version <= self.vfs_config_version);

                self.vfs_progress_config_version = config_version;
                self.vfs_progress_n_total = n_total;
                self.vfs_progress_n_done = n_done;

                let state = if n_done == 0 {
                    Progress::Begin
                } else if n_done < n_total {
                    Progress::Report
                } else {
                    assert_eq!(n_done, n_total);
                    Progress::End
                };

                self.report_progress(
                    "Roots Scanning",
                    state,
                    Some(format!("{n_done}/{n_total}")),
                    Some(Progress::fraction(n_done, n_total)),
                    None,
                );
            }
            vfs::loader::Message::Loaded { files } => {
                let vfs = &mut self.vfs.write().0;

                for (path, content) in files {
                    let path = VfsPath::from(path);
                    if !self.mem_docs.contains(&path) {
                        vfs.set_file_contents(path, content);
                    }
                }
            }
        }
    }
}
