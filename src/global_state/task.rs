use std::panic::{self, AssertUnwindSafe};

use crossbeam_channel::Sender;
use utils::thread::{Pool, ThreadIntent};
use vide_lsp_runtime::ProtocolTask;

use super::{
    diagnostics::publisher::PublishDiagnosticsBatch,
    qihe::{QiheRunId, QiheUpdate},
    reload::FetchWorkspaceProgress,
    response_effect::AcceptedResponseEffect,
    semantic_compiler::{SemanticCompilerRunId, SemanticCompilerUpdate},
};

#[derive(Debug)]
pub(crate) enum Task {
    Protocol(ProtocolTask<AcceptedResponseEffect>),
    FetchWorkspace(FetchWorkspaceProgress),
    Diagnostics(PublishDiagnosticsBatch),
    Qihe(QiheTask),
    SemanticCompiler(SemanticCompilerTask),
}

impl Task {
    pub(in crate::global_state) fn kind(&self) -> &'static str {
        match self {
            Task::Protocol(ProtocolTask::Response { .. }) => "task.response",
            Task::Protocol(ProtocolTask::Retry(_)) => "task.retry",
            Task::FetchWorkspace(FetchWorkspaceProgress::Begin { .. }) => {
                "task.fetch_workspace.begin"
            }
            Task::FetchWorkspace(FetchWorkspaceProgress::End { .. }) => "task.fetch_workspace.end",
            Task::Diagnostics(_) => "task.diagnostics",
            Task::Qihe(task) => task.kind(),
            Task::SemanticCompiler(task) => task.kind(),
        }
    }

    pub(in crate::global_state) fn summary(&self) -> String {
        match self {
            Task::Protocol(ProtocolTask::Response { response, accepted_effects }) => format!(
                "task response id={:?} error={} accepted_effects={}",
                response.id,
                response.error.is_some(),
                accepted_effects.len()
            ),
            Task::Protocol(ProtocolTask::Retry(request)) => {
                format!("task retry method={} id={:?}", request.method, request.id)
            }
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
            Task::SemanticCompiler(task) => task.summary(),
        }
    }
}

pub(crate) struct TaskPool<T> {
    pub(crate) sender: Sender<T>,
    pub(crate) pool: Pool,
}

impl<T> TaskPool<T> {
    pub(crate) fn new_with_threads_num(sender: Sender<T>, threads_num: usize) -> TaskPool<T> {
        TaskPool { sender, pool: Pool::new(threads_num) }
    }

    pub(crate) fn spawn_and_send<F>(&mut self, intent: ThreadIntent, task: F)
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.pool.spawn(intent, {
            let sender = self.sender.clone();
            move || match panic::catch_unwind(AssertUnwindSafe(task)) {
                Ok(task) => {
                    if sender.send(task).is_err() {
                        tracing::debug!("task result dropped because main loop receiver is closed");
                    }
                }
                Err(panic) => log_task_panic(panic),
            }
        })
    }

    pub(crate) fn spawn_and_send_cps<F>(&mut self, intent: ThreadIntent, task: F)
    where
        F: FnOnce(Sender<T>) + Send + 'static,
        T: Send + 'static,
    {
        self.pool.spawn(intent, {
            let sender = self.sender.clone();
            move || {
                if let Err(panic) = panic::catch_unwind(AssertUnwindSafe(|| task(sender))) {
                    log_task_panic(panic);
                }
            }
        })
    }
}

fn log_task_panic(panic: Box<dyn std::any::Any + Send>) {
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic payload");
    tracing::error!(message, "background task panicked");
}

#[derive(Debug)]
pub(crate) enum QiheTask {
    Log { run_id: QiheRunId, token: String, message: String },
    Finished { run_id: QiheRunId, update: QiheUpdate, progress_token: String },
    Cancelled { run_id: QiheRunId, message: String, progress_token: String },
    Failed { run_id: QiheRunId, message: String, progress_token: String },
}

impl QiheTask {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            QiheTask::Log { .. } => "task.qihe.log",
            QiheTask::Finished { .. } => "task.qihe.finished",
            QiheTask::Cancelled { .. } => "task.qihe.cancelled",
            QiheTask::Failed { .. } => "task.qihe.failed",
        }
    }

    pub(super) fn summary(&self) -> String {
        match self {
            QiheTask::Log { token, message, .. } => {
                format!("task qihe log token={token} bytes={}", message.len())
            }
            QiheTask::Finished { progress_token, .. } => {
                format!("task qihe finished token={progress_token}")
            }
            QiheTask::Cancelled { progress_token, message, .. } => {
                format!("task qihe cancelled token={progress_token} message={message}")
            }
            QiheTask::Failed { progress_token, message, .. } => {
                format!("task qihe failed token={progress_token} message={message}")
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum SemanticCompilerTask {
    Finished { run_id: SemanticCompilerRunId, update: SemanticCompilerUpdate },
    Cancelled { run_id: SemanticCompilerRunId },
    Failed { run_id: SemanticCompilerRunId, message: String },
}

impl SemanticCompilerTask {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            SemanticCompilerTask::Finished { .. } => "task.semantic_compiler.finished",
            SemanticCompilerTask::Cancelled { .. } => "task.semantic_compiler.cancelled",
            SemanticCompilerTask::Failed { .. } => "task.semantic_compiler.failed",
        }
    }

    pub(super) fn summary(&self) -> String {
        match self {
            SemanticCompilerTask::Finished { run_id, update } => {
                format!(
                    "task semantic compiler finished run={run_id:?} files={} diagnostics={}",
                    update.touched_file_count(),
                    update.diagnostic_count()
                )
            }
            SemanticCompilerTask::Cancelled { run_id } => {
                format!("task semantic compiler cancelled run={run_id:?}")
            }
            SemanticCompilerTask::Failed { run_id, message } => {
                format!("task semantic compiler failed run={run_id:?} message={message}")
            }
        }
    }
}
