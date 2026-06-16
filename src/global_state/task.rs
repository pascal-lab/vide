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
