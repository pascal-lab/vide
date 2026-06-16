use std::{
    collections::HashMap,
    panic::{self, AssertUnwindSafe},
};

use crossbeam_channel::Sender;
use utils::{
    cancellation::CancellationToken,
    thread::{Pool, ThreadIntent},
};

use super::{
    main_loop::PublishDiagnosticsBatch,
    qihe::{QiheRunId, QiheUpdate},
    reload::FetchWorkspaceProgress,
    response_effect::AcceptedResponseEffect,
};

#[derive(Debug)]
pub(crate) enum Task {
    Response(ResponseTask),
    Retry(lsp_server::Request),
    FetchWorkspace(FetchWorkspaceProgress),
    Diagnostics(PublishDiagnosticsBatch),
    Qihe(QiheTask),
}

#[derive(Debug)]
pub(crate) struct ResponseTask {
    pub(super) response: lsp_server::Response,
    pub(super) accepted_effects: Vec<AcceptedResponseEffect>,
}

impl ResponseTask {
    pub(crate) fn new(response: lsp_server::Response) -> Self {
        Self { response, accepted_effects: Vec::new() }
    }

    pub(crate) fn with_accepted_effects(
        mut self,
        accepted_effects: Vec<AcceptedResponseEffect>,
    ) -> Self {
        self.accepted_effects = accepted_effects;
        self
    }

    pub(super) fn summary(&self) -> String {
        format!(
            "task response id={:?} error={} accepted_effects={}",
            self.response.id,
            self.response.error.is_some(),
            self.accepted_effects.len()
        )
    }
}

pub(crate) struct TaskPool<T> {
    pub(crate) sender: Sender<T>,
    pub(crate) pool: Pool,
    lifecycle_cancel: CancellationToken,
    request_cancel_tokens: HashMap<lsp_server::RequestId, CancellationToken>,
}

impl<T> TaskPool<T> {
    pub(crate) fn new_with_threads_num(sender: Sender<T>, threads_num: usize) -> TaskPool<T> {
        TaskPool {
            sender,
            pool: Pool::new(threads_num),
            lifecycle_cancel: CancellationToken::new(),
            request_cancel_tokens: HashMap::new(),
        }
    }

    pub(crate) fn task_token(&self) -> CancellationToken {
        self.lifecycle_cancel.child_token()
    }

    pub(crate) fn register_request(
        &mut self,
        request_id: lsp_server::RequestId,
    ) -> CancellationToken {
        let token = self.task_token();
        self.request_cancel_tokens.insert(request_id, token.clone());
        token
    }

    pub(crate) fn request_token(
        &self,
        request_id: &lsp_server::RequestId,
    ) -> Option<CancellationToken> {
        self.request_cancel_tokens.get(request_id).cloned()
    }

    pub(crate) fn complete_request(&mut self, request_id: &lsp_server::RequestId) {
        self.request_cancel_tokens.remove(request_id);
    }

    pub(crate) fn cancel_request(&mut self, request_id: &lsp_server::RequestId) {
        if let Some(token) = self.request_cancel_tokens.remove(request_id) {
            token.cancel();
        }
    }

    pub(crate) fn cancel_all(&mut self) {
        self.lifecycle_cancel.cancel();
        self.request_cancel_tokens.clear();
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

#[cfg(test)]
mod tests {
    use super::TaskPool;

    #[test]
    fn task_pool_request_cancel_signals_registered_token() {
        let (sender, _receiver) = crossbeam_channel::unbounded::<()>();
        let mut pool = TaskPool::new_with_threads_num(sender, 0);
        let request_id = lsp_server::RequestId::from(7);
        let token = pool.register_request(request_id.clone());

        pool.cancel_request(&request_id);

        assert!(token.is_cancelled());
        assert!(pool.request_token(&request_id).is_none());
    }

    #[test]
    fn task_pool_lifecycle_cancel_signals_child_tokens() {
        let (sender, _receiver) = crossbeam_channel::unbounded::<()>();
        let mut pool = TaskPool::new_with_threads_num(sender, 0);
        let token = pool.task_token();

        pool.cancel_all();

        assert!(token.is_cancelled());
    }
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
