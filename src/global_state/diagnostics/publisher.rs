use rustc_hash::{FxHashMap, FxHashSet};
use vfs::FileId;

use super::DiagnosticPublishFreshness;
use crate::{
    config::Config,
    global_state::{
        GlobalState, snapshot::DiagnosticPublishTarget, workspace_state::WorkspaceVfsReadiness,
    },
};

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

    pub(crate) fn touched_file_count(&self) -> usize {
        self.touched_file_ids.len()
    }

    pub(crate) fn diagnostic_count(&self) -> usize {
        self.tasks.iter().map(|task| task.diagnostics.len()).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct DiagnosticPublishKey {
    /// Diagnostics are computed per analysis file but delivered per LSP URI.
    /// Keeping both parts in the cache key prevents an alias URI from being
    /// skipped just because the same diagnostic list was sent to another URI.
    pub(super) file_id: FileId,
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

pub(in crate::global_state) struct DiagnosticsPublisher<'a> {
    config: &'a Config,
    workspace_vfs: &'a mut WorkspaceVfsReadiness,
    published_diagnostics: &'a mut FxHashMap<DiagnosticPublishKey, Vec<lsp_types::Diagnostic>>,
    sender: &'a crossbeam_channel::Sender<lsp_server::Message>,
    current_freshness: DiagnosticPublishFreshness,
}

impl<'a> DiagnosticsPublisher<'a> {
    pub(in crate::global_state) fn new(
        config: &'a Config,
        workspace_vfs: &'a mut WorkspaceVfsReadiness,
        published_diagnostics: &'a mut FxHashMap<DiagnosticPublishKey, Vec<lsp_types::Diagnostic>>,
        sender: &'a crossbeam_channel::Sender<lsp_server::Message>,
        current_freshness: DiagnosticPublishFreshness,
    ) -> Self {
        Self { config, workspace_vfs, published_diagnostics, sender, current_freshness }
    }

    pub(in crate::global_state) fn publish(&mut self, batch: PublishDiagnosticsBatch) {
        let task_count = batch.touched_file_count();
        let diagnostic_count = batch.diagnostic_count();
        let _span =
            tracing::info_span!("diagnostics.publish", task_count, diagnostic_count).entered();

        if self.config.cli_pull_diagnostics_support() {
            tracing::info!("skipping push diagnostics for pull-capable client");
            return;
        }

        if !self.workspace_vfs.is_ready() {
            self.workspace_vfs.defer_diagnostics_until_ready();
            tracing::debug!("diagnostics publish deferred until workspace/VFS is ready");
            return;
        }

        let mut published_files = 0usize;
        let mut published_diagnostics = 0usize;
        let mut skipped_files = 0usize;
        let PublishDiagnosticsBatch { freshness, touched_file_ids, tasks } = batch;
        if freshness != self.current_freshness {
            tracing::debug!(
                freshness = ?freshness,
                current_freshness = ?self.current_freshness,
                "stale diagnostics batch ignored"
            );
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

    fn send_notification<N: lsp_types::notification::Notification>(&self, params: N::Params) {
        let notif = lsp_server::Notification::new(N::METHOD.to_string(), params);
        if self.sender.send(notif.into()).is_err() {
            tracing::debug!("LSP message dropped because client connection is closed");
        }
    }
}

impl GlobalState {
    pub(in crate::global_state) fn publish_diagnostics_tasks(
        &mut self,
        batch: PublishDiagnosticsBatch,
    ) {
        let current_freshness = self.diagnostic_publish_freshness();
        DiagnosticsPublisher::new(
            &self.config_state.config,
            &mut self.workspace.workspace_vfs,
            &mut self.diagnostics.published_diagnostics,
            &self.client.sender,
            current_freshness,
        )
        .publish(batch);
    }
}
