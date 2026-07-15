use std::{
    panic::{self, AssertUnwindSafe},
    sync::Arc as StdArc,
};

use anyhow::{Context, Result};
use hir::base_db::project::CompilationProfileId;
use rustc_hash::FxHashSet;
use utils::{
    cancellation::{CancellationError, CancellationToken},
    thread::ThreadIntent,
};
use vfs::FileId;

use super::{
    AnalysisState, ClientState, ConfigState, DEFAULT_REQ_HANDLER, DiagnosticsState, GlobalState,
    TaskState, WorkspaceState,
    diagnostics::{
        DiagnosticPublishFreshness, DiagnosticSource,
        publisher::{DiagnosticsPublisher, PublishDiagnosticsBatch, PublishDiagnosticsTask},
    },
    snapshot::GlobalStateSnapshot,
    task::{SemanticCompilerTask, Task},
};

#[derive(Debug)]
pub(crate) struct SemanticCompilerUpdate {
    delivery: SemanticDiagnosticsDelivery,
    touched_files: FxHashSet<FileId>,
    diagnostic_count: usize,
    freshness: DiagnosticPublishFreshness,
}

#[derive(Debug)]
enum SemanticDiagnosticsDelivery {
    PullRefresh,
    Push(PublishDiagnosticsBatch),
}

impl SemanticCompilerUpdate {
    pub(crate) fn touched_file_count(&self) -> usize {
        self.touched_files.len()
    }

    pub(crate) fn diagnostic_count(&self) -> usize {
        self.diagnostic_count
    }
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub(crate) struct SemanticCompilerRunId(u64);

impl SemanticCompilerRunId {
    fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

pub(crate) struct SemanticCompiler {
    run_generation: SemanticCompilerRunId,
    active_cancel_token: Option<CancellationToken>,
    pending_profiles: FxHashSet<CompilationProfileId>,
}

impl SemanticCompiler {
    pub(crate) fn new() -> Self {
        Self {
            run_generation: SemanticCompilerRunId::default(),
            active_cancel_token: None,
            pending_profiles: FxHashSet::default(),
        }
    }

    pub(crate) fn schedule<C: SemanticCompilerCtx>(
        &mut self,
        profile_ids: Vec<CompilationProfileId>,
        ctx: &mut C,
    ) {
        let profile_ids = normalize_profile_ids(profile_ids);
        if profile_ids.is_empty() {
            return;
        }

        if self.active_cancel_token.is_some() {
            self.pending_profiles.extend(profile_ids);
            return;
        }

        self.start_run(profile_ids, ctx);
    }

    pub(crate) fn handle<C: SemanticCompilerCtx>(
        &mut self,
        task: SemanticCompilerTask,
        ctx: &mut C,
    ) {
        match task {
            SemanticCompilerTask::Finished { run_id, update } => {
                if run_id != self.run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.run_generation,
                        "stale semantic compiler result ignored"
                    );
                    return;
                }

                if self.active_cancel_token.as_ref().is_some_and(CancellationToken::is_cancelled) {
                    self.active_cancel_token = None;
                    self.start_pending(ctx);
                    return;
                }

                self.active_cancel_token = None;
                let current_freshness = ctx.diagnostic_publish_freshness();
                if update.freshness != current_freshness {
                    tracing::debug!(
                        ?run_id,
                        freshness = ?update.freshness,
                        current = ?current_freshness,
                        "stale semantic compiler diagnostics ignored"
                    );
                    self.start_pending(ctx);
                    return;
                }

                let SemanticCompilerUpdate { delivery, touched_files, .. } = update;
                match delivery {
                    SemanticDiagnosticsDelivery::PullRefresh => {
                        ctx.refresh_semantic_diagnostics(touched_files);
                    }
                    SemanticDiagnosticsDelivery::Push(batch) => {
                        ctx.publish_semantic_diagnostics(batch);
                    }
                }
                self.start_pending(ctx);
            }
            SemanticCompilerTask::Cancelled { run_id } => {
                if run_id != self.run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.run_generation,
                        "stale semantic compiler cancellation ignored"
                    );
                    return;
                }
                self.active_cancel_token = None;
                self.start_pending(ctx);
            }
            SemanticCompilerTask::Failed { run_id, message } => {
                if run_id != self.run_generation {
                    tracing::debug!(
                        ?run_id,
                        current = ?self.run_generation,
                        "stale semantic compiler failure ignored"
                    );
                    return;
                }
                self.active_cancel_token = None;
                tracing::warn!(message, "semantic compiler diagnostics failed");
                self.start_pending(ctx);
            }
        }
    }

    fn start_run<C: SemanticCompilerCtx>(
        &mut self,
        profile_ids: Vec<CompilationProfileId>,
        ctx: &mut C,
    ) {
        self.run_generation = self.run_generation.next();
        let run_id = self.run_generation;
        let cancellation = ctx.task_cancel_token();
        let snapshot = ctx.make_snapshot(cancellation.clone());
        self.active_cancel_token = Some(cancellation.clone());

        ctx.spawn_semantic_compiler_task(move |sender| {
            let task = Task::SemanticCompiler(
                panic::catch_unwind(AssertUnwindSafe(|| {
                    run_semantic_compiler_task(snapshot, profile_ids, run_id, cancellation)
                }))
                .unwrap_or_else(|panic| {
                    let message = panic_message(&panic)
                        .map(|message| format!("semantic compiler panicked: {message}"))
                        .unwrap_or_else(|| "semantic compiler panicked".to_owned());
                    SemanticCompilerTask::Failed { run_id, message }
                }),
            );
            if sender.send(task).is_err() {
                tracing::debug!(
                    "semantic compiler result dropped because main loop receiver is closed"
                );
            }
        });
    }

    fn start_pending<C: SemanticCompilerCtx>(&mut self, ctx: &mut C) {
        let profile_ids = self.pending_profiles.drain().collect::<Vec<_>>();
        let profile_ids = normalize_profile_ids(profile_ids);
        if !profile_ids.is_empty() {
            self.start_run(profile_ids, ctx);
        }
    }
}

pub(crate) trait SemanticCompilerCtx {
    fn diagnostic_publish_freshness(&self) -> DiagnosticPublishFreshness;
    fn make_snapshot(&self, cancellation: CancellationToken) -> GlobalStateSnapshot;
    fn spawn_semantic_compiler_task<F>(&mut self, task: F)
    where
        F: FnOnce(crossbeam_channel::Sender<Task>) + Send + 'static;
    fn task_cancel_token(&self) -> CancellationToken;
    fn refresh_semantic_diagnostics(&mut self, changed_files: FxHashSet<FileId>);
    fn publish_semantic_diagnostics(&mut self, batch: PublishDiagnosticsBatch);
}

pub(super) struct SemanticCompilerGlobalCtx<'a> {
    client: &'a mut ClientState,
    config_state: &'a mut ConfigState,
    analysis: &'a mut AnalysisState,
    diagnostics: &'a mut DiagnosticsState,
    workspace: &'a mut WorkspaceState,
    external_sources: &'a [StdArc<dyn DiagnosticSource>],
    tasks: &'a mut TaskState,
}

impl SemanticCompilerGlobalCtx<'_> {
    fn send(&self, message: lsp_server::Message) {
        if self.client.sender.send(message).is_err() {
            tracing::debug!("LSP message dropped because client connection is closed");
        }
    }

    fn send_request<R: lsp_types::request::Request>(&mut self, params: R::Params) {
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
                "semantic diagnostics refresh deferred until workspace/VFS is ready"
            );
            return;
        }

        if self.config_state.config.cli_workspace_diagnostic_refresh_support() {
            self.send_request::<lsp_types::request::WorkspaceDiagnosticRefresh>(());
        }
    }
}

impl SemanticCompilerCtx for SemanticCompilerGlobalCtx<'_> {
    fn diagnostic_publish_freshness(&self) -> DiagnosticPublishFreshness {
        super::diagnostic_publish_freshness(self.analysis, self.diagnostics, self.workspace)
    }

    fn make_snapshot(&self, cancellation: CancellationToken) -> GlobalStateSnapshot {
        super::make_snapshot(
            &self.config_state.config,
            &self.workspace.workspaces,
            self.analysis,
            &self.workspace.vfs,
            self.external_sources,
            self.diagnostics,
            self.workspace,
            cancellation,
        )
    }

    fn spawn_semantic_compiler_task<F>(&mut self, task: F)
    where
        F: FnOnce(crossbeam_channel::Sender<Task>) + Send + 'static,
    {
        self.tasks.task_pool.handle.spawn_and_send_cps(ThreadIntent::Worker, task);
    }

    fn task_cancel_token(&self) -> CancellationToken {
        self.tasks.task_pool.handle.task_token()
    }

    fn refresh_semantic_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
        self.refresh_pull_diagnostics(changed_files);
    }

    fn publish_semantic_diagnostics(&mut self, batch: PublishDiagnosticsBatch) {
        if batch.touched_file_count() == 0 {
            return;
        }

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

pub(super) fn with_global_ctx<T>(
    state: &mut GlobalState,
    f: impl FnOnce(&mut SemanticCompiler, &mut SemanticCompilerGlobalCtx<'_>) -> T,
) -> T {
    let GlobalState {
        client,
        config_state,
        analysis,
        diagnostics,
        workspace,
        qihe: _,
        semantic_compiler,
        external_sources,
        tasks,
    } = state;
    let mut ctx = SemanticCompilerGlobalCtx {
        client,
        config_state,
        analysis,
        diagnostics,
        workspace,
        external_sources,
        tasks,
    };
    f(semantic_compiler, &mut ctx)
}

fn run_semantic_compiler_task(
    snapshot: GlobalStateSnapshot,
    profile_ids: Vec<CompilationProfileId>,
    run_id: SemanticCompilerRunId,
    cancellation: CancellationToken,
) -> SemanticCompilerTask {
    match collect_semantic_diagnostics(snapshot, profile_ids, &cancellation) {
        Ok(update) => SemanticCompilerTask::Finished { run_id, update },
        Err(err) if err.is::<CancellationError>() => SemanticCompilerTask::Cancelled { run_id },
        Err(err) => SemanticCompilerTask::Failed { run_id, message: err.to_string() },
    }
}

fn collect_semantic_diagnostics(
    snapshot: GlobalStateSnapshot,
    profile_ids: Vec<CompilationProfileId>,
    cancellation: &CancellationToken,
) -> Result<SemanticCompilerUpdate> {
    let freshness = snapshot.diagnostic_publish_freshness;
    let mut touched_files = FxHashSet::default();
    let mut diagnostic_count = 0;
    let profile_count = profile_ids.len();

    for profile_id in profile_ids {
        cancellation.check()?;
        touched_files.extend(snapshot.analysis.compilation_profile_file_ids(profile_id)?);
        let diagnostics = snapshot.analysis.compilation_profile_diagnostics(profile_id)?;
        diagnostic_count += diagnostics.len();
        cancellation.check()?;
    }
    cancellation.check()?;

    tracing::debug!(
        snapshot_id = ?snapshot.analysis_snapshot_id(),
        profile_count,
        root_file_count = touched_files.len(),
        diagnostic_count,
        "semantic compiler prewarmed profile diagnostics"
    );

    let delivery = if snapshot.config.cli_pull_diagnostics_support() {
        SemanticDiagnosticsDelivery::PullRefresh
    } else {
        SemanticDiagnosticsDelivery::Push(materialize_semantic_publish_batch(
            &snapshot,
            &touched_files,
            cancellation,
        )?)
    };
    drop(snapshot);

    Ok(SemanticCompilerUpdate { delivery, touched_files, diagnostic_count, freshness })
}

fn materialize_semantic_publish_batch(
    snapshot: &GlobalStateSnapshot,
    changed_files: &FxHashSet<FileId>,
    cancellation: &CancellationToken,
) -> Result<PublishDiagnosticsBatch> {
    let mut publish_tasks = Vec::with_capacity(changed_files.len());
    let mut touched_file_ids = FxHashSet::default();
    for file_id in changed_files.iter().copied() {
        cancellation.check()?;
        let targets = snapshot
            .diagnostic_publish_targets(file_id)
            .with_context(|| format!("failed to resolve diagnostic targets for {file_id:?}"))?;
        let diagnostics = match snapshot.lsp_diagnostics(file_id) {
            Ok(diagnostics) => diagnostics,
            Err(error) if error.is::<ide::Cancelled>() => return Err(CancellationError.into()),
            Err(error) => {
                return Err(error.context(format!(
                    "failed to materialize semantic diagnostics for {file_id:?}"
                )));
            }
        };
        touched_file_ids.insert(file_id);
        publish_tasks.extend(
            targets
                .into_iter()
                .map(|target| PublishDiagnosticsTask::from_target(target, diagnostics.clone())),
        );
    }
    cancellation.check()?;

    Ok(PublishDiagnosticsBatch::for_touched_files(
        touched_file_ids,
        publish_tasks,
        snapshot.diagnostic_publish_freshness,
    ))
}

fn normalize_profile_ids(mut profile_ids: Vec<CompilationProfileId>) -> Vec<CompilationProfileId> {
    profile_ids.sort_unstable_by_key(|profile_id| profile_id.0);
    profile_ids.dedup();
    profile_ids
}

fn panic_message(panic: &(dyn std::any::Any + Send)) -> Option<&str> {
    panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use hir::base_db::change::Change;
    use lsp_server::Connection;
    use lsp_types::{ClientCapabilities, TraceValue};
    use utils::test_support::TestDir;

    use super::*;
    use crate::{
        Opt,
        config::{self, user_config::UserConfig},
        i18n::I18n,
    };

    #[test]
    fn semantic_compiler_task_does_not_retain_analysis_snapshot() {
        let root = TestDir::new("semantic-compiler-snapshot-lifetime");
        let root_path = root.path().to_path_buf();
        let config = config::Config::new(
            Opt {
                process_name: "vide-test".to_owned(),
                log: "error".to_owned(),
                log_filename: None,
                profile_trace: None,
            },
            root_path.clone(),
            ClientCapabilities::default(),
            vec![root_path],
            I18n::default(),
            UserConfig::default(),
            Vec::new(),
        );
        let (server, _client) = Connection::memory();
        let mut state = GlobalState::new(server.sender, config, TraceValue::Off);
        let cancellation = CancellationToken::new();
        let snapshot = state.make_snapshot_with_cancel(cancellation.clone());
        let task = run_semantic_compiler_task(
            snapshot,
            Vec::new(),
            SemanticCompilerRunId::default(),
            cancellation,
        );

        let mut analysis_host = std::mem::take(&mut state.analysis.analysis_host);
        let (finished_tx, finished_rx) = crossbeam_channel::bounded(1);
        let writer = std::thread::spawn(move || {
            analysis_host.apply_change(Change::new());
            finished_tx.send(()).unwrap();
            analysis_host
        });

        let completed_while_task_was_retained =
            finished_rx.recv_timeout(Duration::from_secs(1)).is_ok();
        drop(task);
        state.analysis.analysis_host = writer.join().unwrap();

        assert!(
            completed_while_task_was_retained,
            "semantic compiler task retained an analysis snapshot and blocked the next change"
        );
    }
}
