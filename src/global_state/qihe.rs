use std::{
    ffi::OsStr,
    fs,
    io::{BufRead, BufReader, Read},
    panic::{self, AssertUnwindSafe},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc as StdArc, LazyLock},
    thread::{self, JoinHandle},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, bail};
use hir::base_db::compilation_plan::CompilationPlan;
use ide::FileRange;
use lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, NumberOrString, notification,
    request,
};
use parking_lot::{Mutex, MutexGuard};
use project_model::project_manifest::{ProjectManifest, ProjectManifestFileName};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;
use triomphe::Arc;
use utils::{
    cancellation::{CancellationError, CancellationToken},
    line_index::{LineCol, TextRange, TextSize},
    path_identity::PathIdentityIndex,
    paths::{AbsPath, AbsPathBuf},
    process::{configure_process_tree, wait_with_cancellation},
    thread::ThreadIntent,
};
use vfs::FileId;

use super::{
    AnalysisState, ClientState, ConfigState, DEFAULT_REQ_HANDLER, DiagnosticsState, GlobalState,
    QiheDiagnosticState, TaskState, WorkspaceState,
    diagnostics::{
        DiagnosticCommitFreshness, DiagnosticExternalRevision, DiagnosticOwner,
        DiagnosticPublishFreshness, DiagnosticSource,
        publisher::{DiagnosticsPublisher, PublishDiagnosticsBatch, PublishDiagnosticsTask},
    },
    respond::Progress,
    snapshot::GlobalStateSnapshot,
    task::{QiheTask, Task},
};
use crate::{
    config::user_config::QiheConfig,
    i18n::{I18n, keys},
    lsp_ext::{
        ext::{
            QiheLogNotification, QiheLogParams, QiheStatusNotification, QiheStatusParams,
            RunQiheAnalysisParams,
        },
        from_proto, to_proto,
    },
};

#[derive(Debug)]
pub(crate) struct QiheUpdate {
    by_file: FxHashMap<FileId, Vec<Diagnostic>>,
    summary: String,
    freshness: DiagnosticCommitFreshness,
}

const QIHE: &str = "qihe";
const QIHE_OPTIONS_FILE_NAME: &str = "qihe-options.toml";
const QIHE_OPTIONS_RUN_PATH: &str = "./qihe-options.toml";
const QIHE_LOG_BATCH_BYTES: usize = 8 * 1024;

static ANSI_ESCAPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\x1B\[[0-?]*[ -/]*[@-~]").unwrap());

#[derive(Clone)]
pub(crate) struct QiheDiagnostics {
    states: Arc<Mutex<FxHashMap<FileId, QiheDiagnosticState>>>,
}

impl QiheDiagnostics {
    pub(crate) fn new() -> Self {
        Self { states: Arc::new(Mutex::new(FxHashMap::default())) }
    }

    fn lock(&self) -> MutexGuard<'_, FxHashMap<FileId, QiheDiagnosticState>> {
        self.states.lock()
    }
}

impl DiagnosticSource for QiheDiagnostics {
    fn lsp_diagnostics(
        &self,
        file_id: FileId,
        freshness: &DiagnosticCommitFreshness,
    ) -> Vec<Diagnostic> {
        self.lock()
            .get(&file_id)
            .filter(|state| state.freshness == *freshness)
            .map(|state| state.diagnostics.clone())
            .unwrap_or_default()
    }

    fn external_revision(
        &self,
        file_id: FileId,
        freshness: &DiagnosticCommitFreshness,
    ) -> Option<DiagnosticExternalRevision> {
        self.lock().get(&file_id).filter(|state| state.freshness == *freshness).map(|state| {
            DiagnosticExternalRevision::new(
                DiagnosticOwner::External { source: QIHE, file: file_id },
                state.generation,
            )
        })
    }

    fn remove_deleted(&self, files: &FxHashSet<FileId>) {
        if files.is_empty() {
            return;
        }

        let mut diagnostics = self.lock();
        for file_id in files {
            diagnostics.remove(file_id);
        }
    }
}

/// Monotonic identity for a Qihe analysis run.
///
/// The server keeps latest-run semantics for Qihe: logs, diagnostics, and
/// progress completion from older runs are ignored once a newer run starts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
pub(crate) struct QiheRunId(u64);

impl QiheRunId {
    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[cfg(test)]
    fn new(value: u64) -> Self {
        Self(value)
    }
}

fn qihe_progress_token(run_id: QiheRunId, uri: &lsp_types::Url) -> String {
    format!("qihe-analysis:{}:{uri}", run_id.0)
}

#[derive(Clone)]
struct QiheLogSink {
    sender: crossbeam_channel::Sender<Task>,
    run_id: QiheRunId,
    token: String,
}

impl QiheLogSink {
    fn new(sender: crossbeam_channel::Sender<Task>, run_id: QiheRunId, token: String) -> Self {
        Self { sender, run_id, token }
    }

    fn log(&self, message: impl Into<String>) {
        let task = Task::Qihe(QiheTask::Log {
            run_id: self.run_id,
            token: self.token.clone(),
            message: message.into(),
        });
        if self.sender.send(task).is_err() {
            tracing::debug!("qihe log dropped because main loop receiver is closed");
        }
    }
}

impl QiheUpdate {
    fn from_json_diagnostics(
        active_file_id: FileId,
        diagnostics: Vec<QiheJsonDiagnostic>,
        converter: &DiagnosticConverter<'_>,
        freshness: DiagnosticCommitFreshness,
        cancellation: &CancellationToken,
    ) -> Result<Self> {
        let total = diagnostics.len();
        let mut by_file = FxHashMap::from_iter([(active_file_id, Vec::new())]);

        for diagnostic in diagnostics {
            cancellation.check()?;
            let (file_id, diagnostic) = converter.convert(diagnostic).context(
                converter.snapshot.config.i18n.text(keys::QIHE_CONVERT_DIAGNOSTIC_FAILED),
            )?;
            by_file.entry(file_id).or_default().push(diagnostic);
        }
        cancellation.check()?;

        let summary = converter
            .snapshot
            .config
            .i18n
            .format(keys::QIHE_FINISHED, [("total", total.to_string())]);
        Ok(Self { by_file, summary, freshness })
    }
}

pub(crate) struct Qihe {
    // Only the latest Qihe run is allowed to commit diagnostics or logs.
    run_generation: QiheRunId,
    active_progress_token: Option<String>,
    active_cancel_token: Option<CancellationToken>,
    diagnostics: QiheDiagnostics,
}

impl Qihe {
    pub(crate) fn new(diagnostics: QiheDiagnostics) -> Self {
        Self {
            run_generation: QiheRunId::default(),
            active_progress_token: None,
            active_cancel_token: None,
            diagnostics,
        }
    }

    pub(crate) fn diagnostics_snapshot(&self) -> QiheDiagnostics {
        self.diagnostics.clone()
    }

    pub(crate) fn start<C: QiheCtx>(&mut self, params: RunQiheAnalysisParams, ctx: &mut C) {
        self.end_superseded(ctx);
        self.run_generation = self.run_generation.next();
        let run_id = self.run_generation;
        let progress_token = qihe_progress_token(run_id, &params.uri);
        let progress_label = params.uri.path().to_string();
        let cancellation = ctx.task_cancel_token();
        let snapshot = ctx.make_snapshot(cancellation.clone());

        self.active_progress_token = Some(progress_token.clone());
        self.active_cancel_token = Some(cancellation.clone());
        begin_qihe_progress(ctx, &progress_token, progress_label);

        ctx.spawn_qihe_task(move |sender| {
            let log_sink = QiheLogSink::new(sender.clone(), run_id, progress_token.clone());
            let task = Task::Qihe(
                panic::catch_unwind(AssertUnwindSafe(|| {
                    run_qihe_task(
                        snapshot,
                        params,
                        run_id,
                        progress_token.clone(),
                        log_sink,
                        cancellation,
                    )
                }))
                .unwrap_or_else(|panic| {
                    let message = panic_message(&panic)
                        .map(|message| format!("Qihe analysis panicked: {message}"))
                        .unwrap_or_else(|| "Qihe analysis panicked".to_owned());
                    QiheTask::Failed { run_id, message, progress_token }
                }),
            );
            if sender.send(task).is_err() {
                tracing::debug!("qihe result dropped because main loop receiver is closed");
            }
        });
    }

    pub(crate) fn handle<C: QiheCtx>(&mut self, task: QiheTask, ctx: &mut C) {
        match task {
            QiheTask::Log { run_id, token, message } => {
                if run_id == self.run_generation {
                    ctx.log_qihe(token, message);
                }
            }
            QiheTask::Finished { run_id, update, progress_token } => {
                if run_id != self.run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.run_generation,
                        "stale qihe result ignored"
                    );
                    return;
                }
                if self.active_cancel_token.as_ref().is_some_and(CancellationToken::is_cancelled)
                    && self.active_progress_token.as_deref() == Some(progress_token.as_str())
                {
                    let message = ctx.i18n_text(QiheI18nKey::Cancelled).to_owned();
                    self.end_current(progress_token, "end", message.clone(), message, ctx);
                    return;
                }
                let current_freshness = ctx.diagnostic_commit_freshness();
                if update.freshness != current_freshness {
                    tracing::debug!(
                        ?run_id,
                        freshness = ?update.freshness,
                        current = ?current_freshness,
                        "stale qihe diagnostics ignored"
                    );
                    let message = ctx.i18n_text(QiheI18nKey::Stale).to_owned();
                    self.end_current(progress_token, "end", message.clone(), message, ctx);
                    return;
                }
                let summary = update.summary.clone();
                let changed_files = self.replace_diagnostics(update.by_file, current_freshness);
                self.publish_diagnostics(changed_files, ctx);
                self.end_current(progress_token, "end", summary.clone(), summary, ctx);
            }
            QiheTask::Cancelled { run_id, message, progress_token } => {
                if run_id != self.run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.run_generation,
                        "stale qihe cancellation ignored"
                    );
                    return;
                }
                self.end_current(progress_token, "end", message.clone(), message, ctx);
            }
            QiheTask::Failed { run_id, message, progress_token } => {
                if run_id != self.run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.run_generation,
                        "stale qihe failure ignored"
                    );
                    return;
                }
                self.end_current(
                    progress_token,
                    "failed",
                    message.clone(),
                    ctx.i18n_text(QiheI18nKey::Failed).to_owned(),
                    ctx,
                );
            }
        }
    }

    pub(crate) fn cancel_active(&mut self) {
        if let Some(cancel) = &self.active_cancel_token {
            cancel.cancel();
        }
    }

    pub(crate) fn cancel_progress_token(&mut self, token: &str) {
        if self.active_progress_token.as_deref() == Some(token) {
            self.cancel_active();
        }
    }

    pub(crate) fn publish_diagnostics<C: QiheCtx>(
        &mut self,
        changed_files: FxHashSet<FileId>,
        ctx: &mut C,
    ) {
        ctx.publish_qihe_diagnostics(changed_files);
    }

    fn end_superseded<C: QiheCtx>(&mut self, ctx: &mut C) {
        self.cancel_active();
        self.active_cancel_token = None;
        let Some(progress_token) = self.active_progress_token.take() else {
            return;
        };

        let message = "Superseded by newer Qihe analysis".to_owned();
        end_qihe_progress(ctx, progress_token, "end", message.clone(), message);
    }

    fn end_current<C: QiheCtx>(
        &mut self,
        progress_token: String,
        state: &str,
        message: String,
        progress_message: String,
        ctx: &mut C,
    ) {
        if self.active_progress_token.as_deref() == Some(progress_token.as_str()) {
            self.active_progress_token = None;
            self.active_cancel_token = None;
        }
        end_qihe_progress(ctx, progress_token, state, message, progress_message);
    }

    fn replace_diagnostics(
        &mut self,
        mut by_file: FxHashMap<FileId, Vec<Diagnostic>>,
        freshness: DiagnosticCommitFreshness,
    ) -> FxHashSet<FileId> {
        let mut cache = self.diagnostics.lock();
        let mut changed_files = cache
            .iter()
            .filter_map(|(&file_id, state)| (!state.diagnostics.is_empty()).then_some(file_id))
            .collect::<FxHashSet<_>>();
        changed_files.extend(by_file.keys().copied());

        for file_id in &changed_files {
            let diagnostics = by_file.remove(file_id).unwrap_or_default();
            let generation =
                cache.get(file_id).map_or(1, |state| state.generation.saturating_add(1));
            cache.insert(*file_id, QiheDiagnosticState { freshness, generation, diagnostics });
        }

        changed_files
    }
}

pub(crate) trait QiheCtx {
    fn i18n_text(&self, key: QiheI18nKey) -> &str;
    fn diagnostic_commit_freshness(&self) -> DiagnosticCommitFreshness;
    fn make_snapshot(&self, cancellation: CancellationToken) -> GlobalStateSnapshot;
    fn spawn_qihe_task<F>(&mut self, task: F)
    where
        F: FnOnce(crossbeam_channel::Sender<Task>) + Send + 'static;
    fn task_cancel_token(&self) -> CancellationToken;
    fn send_qihe_status(&mut self, token: &str, state: &str, message: Option<String>);
    fn log_qihe(&mut self, token: String, message: String);
    fn report_qihe_progress(
        &mut self,
        state: Progress,
        message: String,
        fraction: Option<f64>,
        token: String,
    );
    fn publish_qihe_diagnostics(&mut self, changed_files: FxHashSet<FileId>);
}

#[derive(Clone, Copy)]
pub(crate) enum QiheI18nKey {
    ProgressTitle,
    Cancelled,
    Stale,
    Failed,
}

pub(super) struct QiheGlobalCtx<'a> {
    client: &'a mut ClientState,
    config_state: &'a mut ConfigState,
    analysis: &'a mut AnalysisState,
    diagnostics: &'a mut DiagnosticsState,
    workspace: &'a mut WorkspaceState,
    external_sources: &'a [StdArc<dyn DiagnosticSource>],
    tasks: &'a mut TaskState,
}

impl QiheGlobalCtx<'_> {
    fn diagnostic_publish_freshness(&self) -> DiagnosticPublishFreshness {
        DiagnosticPublishFreshness::new(
            self.analysis.analysis_host.snapshot_id(),
            self.diagnostics.diagnostics_revision,
            self.diagnostics.diagnostic_target_revision,
            self.workspace.workspace_vfs.diagnostic_readiness_revision(),
        )
    }

    fn send(&self, message: lsp_server::Message) {
        if self.client.sender.send(message).is_err() {
            tracing::debug!("LSP message dropped because client connection is closed");
        }
    }

    fn send_notification<N: notification::Notification>(&self, params: N::Params) {
        let notif = lsp_server::Notification::new(N::METHOD.to_string(), params);
        self.send(notif.into());
    }

    fn send_request<R: request::Request>(&mut self, params: R::Params) {
        let request = self.client.req_queue.outgoing.register(
            R::METHOD.to_string(),
            params,
            DEFAULT_REQ_HANDLER,
        );
        self.send(request.into());
    }

    fn refresh_pull_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
        if changed_files.is_empty() {
            return;
        }

        if !self.workspace.workspace_vfs.is_ready() {
            self.workspace.workspace_vfs.defer_diagnostics_until_ready();
            tracing::debug!(
                ?changed_files,
                "diagnostics invalidation deferred until workspace/VFS is ready"
            );
            return;
        }

        if self.config_state.config.cli_workspace_diagnostic_refresh_support() {
            self.send_request::<request::WorkspaceDiagnosticRefresh>(());
        }
    }
}

impl QiheCtx for QiheGlobalCtx<'_> {
    fn i18n_text(&self, key: QiheI18nKey) -> &str {
        let key = match key {
            QiheI18nKey::ProgressTitle => keys::QIHE_PROGRESS_TITLE,
            QiheI18nKey::Cancelled => keys::QIHE_CANCELLED,
            QiheI18nKey::Stale => keys::QIHE_STALE,
            QiheI18nKey::Failed => keys::QIHE_FAILED,
        };
        self.config_state.config.i18n.text(key)
    }

    fn diagnostic_commit_freshness(&self) -> DiagnosticCommitFreshness {
        self.diagnostic_publish_freshness().commit()
    }

    fn make_snapshot(&self, cancellation: CancellationToken) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            config: Arc::clone(&self.config_state.config),
            workspaces: Arc::clone(&self.workspace.workspaces),
            analysis: self.analysis.analysis_host.make_analysis(),
            vfs: Arc::clone(&self.workspace.vfs),
            mem_docs: self.analysis.mem_docs.clone(),
            sema_tokens_cache: Arc::clone(&self.analysis.semantic_tokens_cache),
            external_sources: self.external_sources.to_vec(),
            diagnostic_publish_freshness: self.diagnostic_publish_freshness(),
            diagnostic_file_revisions: self.diagnostics.diagnostic_file_revisions.clone(),
            cancellation,
            accepted_response_effects: Default::default(),
        }
    }

    fn spawn_qihe_task<F>(&mut self, task: F)
    where
        F: FnOnce(crossbeam_channel::Sender<Task>) + Send + 'static,
    {
        self.tasks.task_pool.handle.spawn_and_send_cps(ThreadIntent::Worker, task);
    }

    fn task_cancel_token(&self) -> CancellationToken {
        self.tasks.task_pool.handle.task_token()
    }

    fn send_qihe_status(&mut self, token: &str, state: &str, message: Option<String>) {
        self.send_notification::<QiheStatusNotification>(QiheStatusParams {
            token: token.to_owned(),
            state: state.to_owned(),
            message,
        });
    }

    fn log_qihe(&mut self, token: String, message: String) {
        self.send_notification::<QiheLogNotification>(QiheLogParams { token, message });
    }

    fn report_qihe_progress(
        &mut self,
        state: Progress,
        message: String,
        fraction: Option<f64>,
        token: String,
    ) {
        if !self.config_state.config.cli_work_done_progress() {
            return;
        }

        let percentage = fraction.map(|f| {
            assert!((0.0..=1.0).contains(&f));
            (f * 100.0) as u32
        });

        let cancellable = Some(true);
        let token = lsp_types::ProgressToken::String(token);
        let title = self.i18n_text(QiheI18nKey::ProgressTitle).to_owned();
        let work_done_progress = match state {
            Progress::Begin => {
                self.send_request::<request::WorkDoneProgressCreate>(
                    lsp_types::WorkDoneProgressCreateParams { token: token.clone() },
                );

                lsp_types::WorkDoneProgress::Begin(lsp_types::WorkDoneProgressBegin {
                    title,
                    cancellable,
                    message: Some(message),
                    percentage,
                })
            }
            Progress::Report => {
                lsp_types::WorkDoneProgress::Report(lsp_types::WorkDoneProgressReport {
                    cancellable,
                    message: Some(message),
                    percentage,
                })
            }
            Progress::End => lsp_types::WorkDoneProgress::End(lsp_types::WorkDoneProgressEnd {
                message: Some(message),
            }),
        };

        self.send_notification::<notification::Progress>(lsp_types::ProgressParams {
            token,
            value: lsp_types::ProgressParamsValue::WorkDone(work_done_progress),
        });
    }

    fn publish_qihe_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
        if changed_files.is_empty() {
            return;
        }

        if self.config_state.config.cli_pull_diagnostics_support() {
            self.refresh_pull_diagnostics(changed_files);
            return;
        }

        let snapshot = self.make_snapshot(self.task_cancel_token());
        let mut publish_tasks = Vec::with_capacity(changed_files.len());
        let mut touched_file_ids = FxHashSet::default();
        for file_id in changed_files.iter().copied() {
            let targets = match snapshot.diagnostic_publish_targets(file_id) {
                Ok(targets) => targets,
                Err(error) => {
                    tracing::debug!(
                        ?file_id,
                        "skipping qihe diagnostics for file without URI: {error:#}"
                    );
                    continue;
                }
            };
            let diagnostics = match snapshot.lsp_diagnostics(file_id) {
                Ok(diagnostics) => diagnostics,
                Err(error) if error.is::<ide::Cancelled>() => {
                    tracing::debug!(?file_id, "qihe diagnostic publish cancelled");
                    continue;
                }
                Err(error) => {
                    tracing::debug!(?file_id, "qihe diagnostic publish failed: {error:#}");
                    continue;
                }
            };
            touched_file_ids.insert(file_id);

            publish_tasks.extend(
                targets
                    .into_iter()
                    .map(|target| PublishDiagnosticsTask::from_target(target, diagnostics.clone())),
            );
        }
        let current_freshness = self.diagnostic_publish_freshness();
        DiagnosticsPublisher::new(
            &self.config_state.config,
            &mut self.workspace.workspace_vfs,
            &mut self.diagnostics.published_diagnostics,
            &self.client.sender,
            current_freshness,
        )
        .publish(PublishDiagnosticsBatch::for_touched_files(
            touched_file_ids,
            publish_tasks,
            snapshot.diagnostic_publish_freshness,
        ));
    }
}

fn begin_qihe_progress<C: QiheCtx>(ctx: &mut C, progress_token: &str, label: String) {
    ctx.send_qihe_status(progress_token, "begin", Some(label.clone()));
    ctx.report_qihe_progress(Progress::Begin, label, None, progress_token.to_owned());
}

fn end_qihe_progress<C: QiheCtx>(
    ctx: &mut C,
    token: String,
    state: &str,
    message: String,
    progress_message: String,
) {
    ctx.send_qihe_status(&token, state, Some(message.clone()));
    ctx.log_qihe(token.clone(), message);
    ctx.report_qihe_progress(Progress::End, progress_message, Some(1.0), token);
}

pub(super) fn with_global_ctx<T>(
    state: &mut GlobalState,
    f: impl FnOnce(&mut Qihe, &mut QiheGlobalCtx<'_>) -> T,
) -> T {
    let GlobalState {
        client,
        config_state,
        analysis,
        diagnostics,
        workspace,
        qihe,
        semantic_compiler: _,
        external_sources,
        tasks,
    } = state;
    let mut ctx = QiheGlobalCtx {
        client,
        config_state,
        analysis,
        diagnostics,
        workspace,
        external_sources,
        tasks,
    };
    f(qihe, &mut ctx)
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> Option<&str> {
    panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
}

fn run_qihe_task(
    snapshot: GlobalStateSnapshot,
    params: RunQiheAnalysisParams,
    run_id: QiheRunId,
    progress_token: String,
    log_sink: QiheLogSink,
    cancellation: CancellationToken,
) -> QiheTask {
    match run_qihe_request(&snapshot, params, &log_sink, &cancellation) {
        Ok(update) => QiheTask::Finished { run_id, update, progress_token },
        Err(err) if err.is::<CancellationError>() => QiheTask::Cancelled {
            run_id,
            message: snapshot.config.i18n.text(keys::QIHE_CANCELLED).to_owned(),
            progress_token,
        },
        Err(err) => QiheTask::Failed { run_id, message: err.to_string(), progress_token },
    }
}

fn run_qihe_request(
    snapshot: &GlobalStateSnapshot,
    params: RunQiheAnalysisParams,
    log_sink: &QiheLogSink,
    cancellation: &CancellationToken,
) -> Result<QiheUpdate> {
    cancellation.check()?;
    let active_path = from_proto::abs_path(&params.uri)?;
    let active_file_id = snapshot.file_id(&params.uri)?;
    let qihe_config = snapshot.config.qihe();
    let cwd = qihe_working_directory(params.cwd, snapshot.config.root_path.as_path());
    let compile_input =
        qihe_compile_input(snapshot, active_file_id, active_path.as_path(), &cwd, cancellation)?;
    let i18n = snapshot.config.i18n;
    let run_plan = resolve_qihe_run_plan(active_path.as_path(), &cwd, &qihe_config.run_args)
        .context(i18n.text(keys::QIHE_PREPARE_WORKSPACE_FAILED))?;
    let command_context = QiheCommandContext { i18n, log_sink, cancellation };
    run_qihe_commands(&qihe_config, &cwd, &compile_input, &run_plan, &command_context)?;
    cancellation.check()?;

    let diagnostics = load_latest_diagnostics(&run_plan.storage_root, i18n)?;
    let resolution_base = if compile_input.uses_manifest() {
        cwd.as_path()
    } else {
        active_path
            .as_path()
            .parent()
            .map(AsRef::as_ref)
            .unwrap_or_else(|| active_path.as_path().as_ref())
    };
    let converter =
        DiagnosticConverter { snapshot, default_file_id: active_file_id, resolution_base };
    let update = QiheUpdate::from_json_diagnostics(
        active_file_id,
        diagnostics,
        &converter,
        snapshot.diagnostic_commit_freshness(),
        cancellation,
    )?;
    cancellation.check()?;
    Ok(update)
}

fn qihe_working_directory(params_cwd: Option<PathBuf>, root_path: &AbsPath) -> PathBuf {
    params_cwd
        .and_then(|path| dunce::canonicalize(path).ok())
        .unwrap_or_else(|| root_path.to_path_buf().into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QiheCompileInput {
    files: Vec<PathBuf>,
    manifest_slang_args: Vec<String>,
    source: QiheCompileInputSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QiheCompileInputSource {
    SingleFile,
    Manifest(ProjectManifestFileName),
}

impl QiheCompileInput {
    fn uses_manifest(&self) -> bool {
        matches!(self.source, QiheCompileInputSource::Manifest(_))
    }
}

fn qihe_compile_input(
    snapshot: &GlobalStateSnapshot,
    active_file_id: FileId,
    active_path: &AbsPath,
    cwd: &Path,
    cancellation: &CancellationToken,
) -> Result<QiheCompileInput> {
    cancellation.check()?;
    let Some(manifest_file_name) =
        qihe_project_manifest_file_name(&snapshot.config.project_manifests, cwd)?
    else {
        return Ok(single_file_qihe_compile_input(active_path));
    };

    cancellation.check()?;
    let plan = snapshot.analysis.compilation_plan(active_file_id).map_err(|_| CancellationError)?;
    cancellation.check()?;
    let files = plan
        .roots
        .iter()
        .filter_map(|file_id| snapshot.file_path(*file_id).map(PathBuf::from))
        .collect::<Vec<_>>();

    Ok(qihe_compile_input_from_plan(&plan, files, active_path, manifest_file_name))
}

fn qihe_project_manifest_file_name(
    manifests: &[ProjectManifest],
    cwd: &Path,
) -> Result<Option<ProjectManifestFileName>> {
    let cwd = AbsPathBuf::try_from(cwd.to_path_buf()).map_err(|path| {
        anyhow!("Qihe working directory must be an absolute UTF-8 path: {}", path.display())
    })?;

    let mut manifest_roots = PathIdentityIndex::default();
    for manifest in manifests {
        let Some(file_name) = manifest.toml_file_name() else {
            continue;
        };
        let Some(root) = project_manifest_workspace_root(manifest) else {
            continue;
        };
        manifest_roots.insert_path(root, file_name);
    }

    Ok(manifest_roots.get_path(cwd.as_path()))
}

fn project_manifest_workspace_root(manifest: &ProjectManifest) -> Option<&AbsPath> {
    match manifest {
        ProjectManifest::Toml(path) => path.parent(),
        ProjectManifest::UnconfiguredRoot(path) => Some(path.as_path()),
    }
}

fn single_file_qihe_compile_input(active_path: &AbsPath) -> QiheCompileInput {
    QiheCompileInput {
        files: vec![active_path.to_path_buf().into()],
        manifest_slang_args: Vec::new(),
        source: QiheCompileInputSource::SingleFile,
    }
}

fn qihe_compile_input_from_plan(
    plan: &CompilationPlan,
    mut files: Vec<PathBuf>,
    active_path: &AbsPath,
    manifest_file_name: ProjectManifestFileName,
) -> QiheCompileInput {
    files.sort();
    files.dedup();

    if files.is_empty() {
        return single_file_qihe_compile_input(active_path);
    }

    let mut slang_args = Vec::new();
    for top_module in &plan.top_modules {
        slang_args.push("--top".to_owned());
        slang_args.push(top_module.clone());
    }
    for include_dir in &plan.include_dirs {
        slang_args.push("-I".to_owned());
        slang_args.push(include_dir.to_string());
    }
    for define in &plan.predefines {
        slang_args.push(format!("-D{define}"));
    }

    QiheCompileInput {
        files,
        manifest_slang_args: slang_args,
        source: QiheCompileInputSource::Manifest(manifest_file_name),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QiheRunPlan {
    ir_path: PathBuf,
    options_path: Option<PathBuf>,
    storage_root: PathBuf,
    append_options_arg: bool,
    append_storage_root_arg: bool,
}

fn resolve_qihe_run_plan(
    active_path: &AbsPath,
    cwd: &Path,
    run_args: &[String],
) -> Result<QiheRunPlan> {
    let millis = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let workspace = std::env::temp_dir()
        .join(QIHE)
        .join(format!("{}-{millis}", active_path.file_stem().unwrap_or("input")));
    let explicit_options_path = explicit_qihe_options_path(run_args, cwd);
    let append_options_arg = explicit_options_path.is_none();
    let options_path = explicit_options_path.or_else(|| {
        let default_options_path = cwd.join(QIHE_OPTIONS_FILE_NAME);
        default_options_path.is_file().then_some(default_options_path)
    });
    let explicit_storage_root = explicit_storage_root_arg(run_args, cwd);
    let options_storage_root =
        options_path.as_deref().map(read_qihe_options_storage_root).transpose()?.flatten();
    let append_storage_root_arg = explicit_storage_root.is_none() && options_storage_root.is_none();
    let storage_root = explicit_storage_root
        .clone()
        .or(options_storage_root)
        .unwrap_or_else(|| workspace.join("storage"));
    fs::create_dir_all(&workspace)?;
    fs::create_dir_all(&storage_root)?;
    Ok(QiheRunPlan {
        ir_path: workspace.join("input.qh"),
        options_path,
        storage_root,
        append_options_arg,
        append_storage_root_arg,
    })
}

fn read_qihe_options_storage_root(options_path: &Path) -> Result<Option<PathBuf>> {
    if !options_path.is_file() {
        return Ok(None);
    }

    let text = fs::read_to_string(options_path)
        .with_context(|| format!("failed to read {}", options_path.display()))?;
    let options: toml::Value = toml::from_str(&text)
        .with_context(|| format!("failed to parse {}", options_path.display()))?;
    Ok(options
        .get("storage")
        .and_then(toml::Value::as_table)
        .and_then(|storage| storage.get("root"))
        .and_then(toml::Value::as_str)
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                options_path.parent().unwrap_or_else(|| Path::new(".")).join(path)
            }
        }))
}

fn explicit_qihe_options_path(run_args: &[String], cwd: &Path) -> Option<PathBuf> {
    let mut options_path = None;
    for (idx, arg) in run_args.iter().enumerate() {
        if arg == "--options"
            && let Some(path) = run_args.get(idx + 1)
        {
            options_path = Some(resolve_qihe_run_arg_path(cwd, path));
        }
    }
    options_path
}

fn explicit_storage_root_arg(run_args: &[String], cwd: &Path) -> Option<PathBuf> {
    let mut storage_root = None;
    for (idx, arg) in run_args.iter().enumerate() {
        if let ("-c", Some(value)) = (arg.as_str(), run_args.get(idx + 1))
            && let Some(path) = value.strip_prefix("storage.root=")
        {
            storage_root = Some(resolve_qihe_run_arg_path(cwd, path));
        }
    }
    storage_root
}

fn resolve_qihe_run_arg_path(cwd: &Path, path: impl AsRef<Path>) -> PathBuf {
    let path = path.as_ref();
    if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) }
}

fn run_qihe_commands(
    qihe_config: &QiheConfig,
    cwd: &Path,
    compile_input: &QiheCompileInput,
    run_plan: &QiheRunPlan,
    command_context: &QiheCommandContext<'_>,
) -> Result<()> {
    let mut command = qihe_command(qihe_config, cwd, "compile");
    prepare_qihe_compile_command(&mut command, qihe_config, compile_input, &run_plan.ir_path);
    command_context.run(&mut command, "qihe compile")?;

    let mut command = qihe_command(qihe_config, cwd, "run");
    prepare_qihe_run_command(&mut command, qihe_config, run_plan);
    command_context.run(&mut command, "qihe run")
}

fn prepare_qihe_compile_command(
    command: &mut Command,
    qihe_config: &QiheConfig,
    compile_input: &QiheCompileInput,
    ir_path: &Path,
) {
    let (qihe_args, user_slang_args) = split_compile_args(&qihe_config.compile_args);
    command.args(&qihe_args);
    let auto_configure_manifest_args =
        qihe_config.auto_configure_args_from_manifest && compile_input.uses_manifest();
    if auto_configure_manifest_args && !has_compile_mode(&qihe_args) {
        command.args(["--mode", "sv"]);
    }
    command.args(&compile_input.files).arg("-o").arg(ir_path);

    let manifest_slang_args = if auto_configure_manifest_args {
        compile_input.manifest_slang_args.as_slice()
    } else {
        &[]
    };
    let has_slang_args = !user_slang_args.is_empty() || !manifest_slang_args.is_empty();
    if has_slang_args {
        command.arg("--").args(&user_slang_args).args(manifest_slang_args);
    }
}

fn prepare_qihe_run_command(
    command: &mut Command,
    qihe_config: &QiheConfig,
    run_plan: &QiheRunPlan,
) {
    command.args(&qihe_config.run_args);
    if run_plan.append_options_arg && run_plan.options_path.is_some() {
        command.args(["--options", QIHE_OPTIONS_RUN_PATH]);
    }
    command.arg("-i").arg(&run_plan.ir_path);
    if run_plan.append_storage_root_arg {
        command.arg("-c").arg(format!("storage.root={}", run_plan.storage_root.display()));
    }
}

fn split_compile_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    let Some(separator) = args.iter().position(|arg| arg == "--") else {
        return (args.to_vec(), Vec::new());
    };
    (args[..separator].to_vec(), args[separator + 1..].to_vec())
}

fn has_compile_mode(args: &[String]) -> bool {
    args.iter().enumerate().any(|(idx, arg)| {
        arg == "--mode"
            || arg.starts_with("--mode=")
            || (arg == "-m" && args.get(idx + 1).is_some())
    })
}

fn qihe_command(qihe_config: &QiheConfig, cwd: &Path, subcommand: &str) -> Command {
    let mut command = Command::new(&qihe_config.command);
    command.current_dir(cwd).arg(subcommand);
    command
}

struct QiheCommandContext<'a> {
    i18n: I18n,
    log_sink: &'a QiheLogSink,
    cancellation: &'a CancellationToken,
}

impl QiheCommandContext<'_> {
    fn run(&self, command: &mut Command, label: &str) -> Result<()> {
        let command_line = command_line(command);
        self.log_sink.log(format!("{label} command:\n{command_line}"));
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        configure_process_tree(command);
        self.cancellation.check()?;
        let mut child = command.spawn().with_context(|| {
            self.i18n.format(
                keys::QIHE_COMMAND_FAILED_TO_START,
                [("label", label.to_owned()), ("command_line", command_line.clone())],
            )
        })?;

        let stdout = child.stdout.take().map(|stdout| {
            stream_command_output(stdout, label.to_owned(), "stdout", self.log_sink.clone())
        });
        let stderr = child.stderr.take().map(|stderr| {
            stream_command_output(stderr, label.to_owned(), "stderr", self.log_sink.clone())
        });

        let status = wait_with_cancellation(&mut child, self.cancellation);
        let stdout = join_command_output(stdout);
        let stderr = join_command_output(stderr);
        let status = status?;
        self.log_sink.log(format!("{label} finished with status {status}"));

        if status.success() {
            return Ok(());
        }

        bail!(
            "{}",
            self.i18n.format(
                keys::QIHE_COMMAND_FAILED,
                [
                    ("label", label.to_owned()),
                    ("status", status.to_string()),
                    ("command_line", command_line),
                    ("stdout", stdout.trim().to_owned()),
                    ("stderr", stderr.trim().to_owned()),
                ],
            )
        );
    }
}

fn stream_command_output<R: Read + Send + 'static>(
    stream: R,
    label: String,
    stream_name: &'static str,
    log_sink: QiheLogSink,
) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut output = String::new();
        let mut log_batch = CommandLogBatch::new(label.clone(), stream_name, log_sink.clone());
        let mut reader = BufReader::new(stream);
        let mut bytes = Vec::new();

        loop {
            bytes.clear();
            let read = match reader.read_until(b'\n', &mut bytes) {
                Ok(read) => read,
                Err(error) => {
                    log_sink.log(format!("{label} {stream_name} read failed: {error}"));
                    break;
                }
            };
            if read == 0 {
                break;
            }

            let chunk = strip_ansi(String::from_utf8_lossy(&bytes).as_ref());
            output.push_str(&chunk);
            log_batch.push_line(&chunk);
        }

        log_batch.flush();
        output
    })
}

struct CommandLogBatch {
    label: String,
    stream_name: &'static str,
    log_sink: QiheLogSink,
    buffer: String,
}

impl CommandLogBatch {
    fn new(label: String, stream_name: &'static str, log_sink: QiheLogSink) -> Self {
        Self { label, stream_name, log_sink, buffer: String::new() }
    }

    fn push_line(&mut self, output: &str) {
        let text = output.trim_end_matches(&['\r', '\n'][..]);
        let line = format!("{} {}: {}", self.label, self.stream_name, text);
        if !self.buffer.is_empty() && self.buffer.len() + line.len() + 1 > QIHE_LOG_BATCH_BYTES {
            self.flush();
        }
        if !self.buffer.is_empty() {
            self.buffer.push('\n');
        }
        self.buffer.push_str(&line);
    }

    fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        self.log_sink.log(std::mem::take(&mut self.buffer));
    }
}

fn join_command_output(handle: Option<JoinHandle<String>>) -> String {
    let Some(handle) = handle else {
        return String::new();
    };
    handle.join().unwrap_or_default()
}

fn command_line(command: &Command) -> String {
    let mut parts = Vec::new();
    if let Some(cwd) = command.get_current_dir() {
        parts.push(format!("cwd={}", quote_command_arg(cwd.as_os_str())));
    }
    parts.push(quote_command_arg(command.get_program()));
    parts.extend(command.get_args().map(quote_command_arg));
    parts.join(" ")
}

fn quote_command_arg(arg: &OsStr) -> String {
    let text = arg.to_string_lossy();
    if !text.is_empty()
        && text.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b':' | b'.' | b'_' | b'-' | b'=' | b'+')
        })
    {
        return text.into_owned();
    }

    format!("{text:?}")
}

fn strip_ansi(text: &str) -> String {
    ANSI_ESCAPE_RE.replace_all(text, "").into_owned()
}

fn load_latest_diagnostics(storage_root: &Path, i18n: I18n) -> Result<Vec<QiheJsonDiagnostic>> {
    let diagnostics_dir = storage_root.join("diagnostics");
    let Some(latest) = latest_diagnostic_path(&diagnostics_dir, i18n)? else {
        return Ok(Vec::new());
    };

    let text = fs::read_to_string(&latest).with_context(|| {
        i18n.format(keys::QIHE_READ_DIAGNOSTICS_FAILED, [("path", latest.display().to_string())])
    })?;
    serde_json::from_str(&text).with_context(|| {
        i18n.format(keys::QIHE_PARSE_DIAGNOSTICS_FAILED, [("path", latest.display().to_string())])
    })
}

fn latest_diagnostic_path(diagnostics_dir: &Path, i18n: I18n) -> Result<Option<PathBuf>> {
    if !diagnostics_dir.exists() {
        return Ok(None);
    }

    let mut latest: Option<((SystemTime, PathBuf), PathBuf)> = None;
    for entry in fs::read_dir(diagnostics_dir).with_context(|| {
        i18n.format(
            keys::QIHE_READ_DIAGNOSTICS_DIR_FAILED,
            [("path", diagnostics_dir.display().to_string())],
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            let modified =
                entry.metadata().and_then(|metadata| metadata.modified()).unwrap_or(UNIX_EPOCH);
            latest = latest.max(Some(((modified, path.clone()), path)));
        }
    }

    Ok(latest.map(|(_, path)| path))
}

struct DiagnosticConverter<'a> {
    snapshot: &'a GlobalStateSnapshot,
    default_file_id: FileId,
    resolution_base: &'a Path,
}

type SourceRange = (FileId, TextRange);

impl<'a> DiagnosticConverter<'a> {
    fn convert(&self, diagnostic: QiheJsonDiagnostic) -> Result<(FileId, Diagnostic)> {
        let QiheJsonDiagnostic { severity, analysis_class, element, message, support_info } =
            diagnostic;

        let mut related_info = Vec::new();
        let mut extra_support_lines = Vec::new();

        for info in &support_info {
            if let Some((file_id, range)) = self.location_from_element(&info.element)? {
                related_info.push(DiagnosticRelatedInformation {
                    location: to_proto::location(self.snapshot, FileRange { file_id, range })
                        .map_err(|_| CancellationError)?,
                    message: info.message.clone(),
                });
            } else {
                extra_support_lines.push(format!("{} ({})", info.message, info.element));
            }
        }

        let (file_id, range, location_unknown) =
            match self.primary_location(&element, &support_info)? {
                Some((file_id, range)) => (file_id, range, false),
                None => (self.default_file_id, TextRange::empty(TextSize::new(0)), true),
            };

        let message = diagnostic_message(
            self.snapshot.config.i18n,
            message,
            &element,
            location_unknown,
            extra_support_lines,
        );
        let line_info = self.snapshot.line_info(file_id).map_err(|_| CancellationError)?;
        let range = to_proto::range(&line_info, range);
        let related_info = (!related_info.is_empty()).then_some(related_info);

        Ok((
            file_id,
            Diagnostic::new(
                range,
                Some(map_severity(&severity)),
                Some(NumberOrString::String(analysis_code(&analysis_class))),
                Some(QIHE.to_owned()),
                message,
                related_info,
                None,
            ),
        ))
    }

    fn primary_location(
        &self,
        element: &str,
        support_info: &[QiheJsonSupportInfo],
    ) -> Result<Option<SourceRange>> {
        std::iter::once(element)
            .chain(support_info.iter().map(|info| info.element.as_str()))
            .find_map(|element| self.location_from_element(element).transpose())
            .transpose()
    }

    fn location_from_element(&self, element: &str) -> Result<Option<SourceRange>> {
        parse_source_loc(element).map_or(Ok(None), |location| self.location_from_source(location))
    }

    fn location_from_source(&self, location: SourceLocation) -> Result<Option<SourceRange>> {
        let file_id = location.file_name.as_deref().map_or(Some(self.default_file_id), |name| {
            resolve_file_name(self.resolution_base, name)
                .and_then(|path| self.snapshot.file_id_for_path(path.as_ref()))
        });
        let Some(file_id) = file_id else { return Ok(None) };

        let line_index =
            self.snapshot.analysis.line_index(file_id).map_err(|_| CancellationError)?;
        let line = location.line.saturating_sub(1);
        let col = location.column.saturating_sub(1);
        let Some(offset) = line_index.offset(LineCol { line, col }) else {
            return Ok(None);
        };

        Ok(Some((file_id, TextRange::empty(offset))))
    }
}

fn diagnostic_message(
    i18n: I18n,
    message: String,
    primary_element: &str,
    location_unknown: bool,
    mut extra_support_lines: Vec<String>,
) -> String {
    if location_unknown && !primary_element.is_empty() {
        extra_support_lines.push(
            i18n.format(keys::QIHE_LOCATION, [("primary_element", primary_element.to_owned())]),
        );
    }
    extra_support_lines.insert(0, message);
    extra_support_lines.join("\n")
}

fn analysis_code(analysis_class: &str) -> String {
    analysis_class.rsplit('.').next().filter(|code| !code.is_empty()).unwrap_or("Qihe").to_owned()
}

fn resolve_file_name(base_dir: &Path, file_name: &str) -> Option<PathBuf> {
    let candidate = Path::new(file_name);
    Some(if candidate.is_absolute() { candidate.to_path_buf() } else { base_dir.join(file_name) })
}

fn map_severity(severity: &str) -> DiagnosticSeverity {
    match severity.trim().to_ascii_uppercase().as_str() {
        "ERROR" => DiagnosticSeverity::ERROR,
        "WARNING" | "WARN" => DiagnosticSeverity::WARNING,
        "INFO" | "INFORMATION" => DiagnosticSeverity::INFORMATION,
        "HINT" => DiagnosticSeverity::HINT,
        _ => DiagnosticSeverity::WARNING,
    }
}

fn parse_source_loc(raw: &str) -> Option<SourceLocation> {
    let raw = raw.trim();
    if matches!(raw.as_bytes().first(), None | Some(b'@' | b'#')) {
        return None;
    }

    let (head, column) = raw.rsplit_once(':')?;
    let column = column.parse().ok()?;
    let Some((file_name, line)) = head.rsplit_once(':') else {
        let line = head.parse().ok()?;
        return Some(SourceLocation { file_name: None, line, column });
    };
    let line = line.parse().ok()?;

    (!file_name.is_empty()).then(|| SourceLocation {
        file_name: Some(file_name.to_string()),
        line,
        column,
    })
}

#[derive(Debug, Clone)]
struct SourceLocation {
    file_name: Option<String>,
    line: u32,
    column: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QiheJsonDiagnostic {
    severity: String,
    analysis_class: String,
    element: String,
    message: String,
    #[serde(default)]
    support_info: Vec<QiheJsonSupportInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QiheJsonSupportInfo {
    element: String,
    message: String,
}

#[cfg(test)]
mod tests;
