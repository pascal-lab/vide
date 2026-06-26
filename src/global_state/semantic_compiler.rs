use std::{
    panic::{self, AssertUnwindSafe},
    sync::Arc as StdArc,
};

use anyhow::Result;
use hir::base_db::project::CompilationProfileId;
use parking_lot::{Mutex, MutexGuard};
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use utils::{
    cancellation::{CancellationError, CancellationToken},
    thread::ThreadIntent,
};
use vfs::FileId;

use super::{
    AnalysisState, ClientState, ConfigState, DEFAULT_REQ_HANDLER, DiagnosticsState, GlobalState,
    TaskState, WorkspaceState,
    diagnostics::{
        DiagnosticCommitFreshness, DiagnosticExternalRevision, DiagnosticOwner,
        DiagnosticPublishFreshness, DiagnosticSource,
        publisher::{DiagnosticsPublisher, PublishDiagnosticsBatch, PublishDiagnosticsTask},
    },
    snapshot::GlobalStateSnapshot,
    task::{SemanticCompilerTask, Task},
};
use crate::lsp_ext::to_proto;

const SLANG_SEMANTIC: &str = "slang-semantic";

#[derive(Debug)]
pub(crate) struct SemanticCompilerUpdate {
    by_file: FxHashMap<FileId, Vec<lsp_types::Diagnostic>>,
    touched_files: FxHashSet<FileId>,
    freshness: DiagnosticCommitFreshness,
}

impl SemanticCompilerUpdate {
    pub(crate) fn touched_file_count(&self) -> usize {
        self.touched_files.len()
    }

    pub(crate) fn diagnostic_count(&self) -> usize {
        self.by_file.values().map(Vec::len).sum()
    }
}

#[derive(Clone)]
pub(crate) struct SemanticDiagnostics {
    states: Arc<Mutex<FxHashMap<FileId, SemanticDiagnosticState>>>,
}

impl SemanticDiagnostics {
    pub(crate) fn new() -> Self {
        Self { states: Arc::new(Mutex::new(FxHashMap::default())) }
    }

    fn lock(&self) -> MutexGuard<'_, FxHashMap<FileId, SemanticDiagnosticState>> {
        self.states.lock()
    }
}

impl DiagnosticSource for SemanticDiagnostics {
    fn diagnostics(
        &self,
        file_id: FileId,
        freshness: &DiagnosticCommitFreshness,
    ) -> Vec<lsp_types::Diagnostic> {
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
                DiagnosticOwner::External { source: SLANG_SEMANTIC, file: file_id },
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
    diagnostics: SemanticDiagnostics,
}

impl SemanticCompiler {
    pub(crate) fn new(diagnostics: SemanticDiagnostics) -> Self {
        Self {
            run_generation: SemanticCompilerRunId::default(),
            active_cancel_token: None,
            pending_profiles: FxHashSet::default(),
            diagnostics,
        }
    }

    pub(crate) fn diagnostics_snapshot(&self) -> SemanticDiagnostics {
        self.diagnostics.clone()
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
                let current_freshness = ctx.diagnostic_commit_freshness();
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

                let changed_files = self.replace_diagnostics(update, current_freshness);
                self.publish_diagnostics(changed_files, ctx);
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

    fn publish_diagnostics<C: SemanticCompilerCtx>(
        &mut self,
        changed_files: FxHashSet<FileId>,
        ctx: &mut C,
    ) {
        ctx.publish_semantic_diagnostics(changed_files);
    }

    fn replace_diagnostics(
        &mut self,
        update: SemanticCompilerUpdate,
        freshness: DiagnosticCommitFreshness,
    ) -> FxHashSet<FileId> {
        let SemanticCompilerUpdate { mut by_file, mut touched_files, freshness: _ } = update;
        touched_files.extend(by_file.keys().copied());

        let mut cache = self.diagnostics.lock();
        let mut changed_files = touched_files
            .iter()
            .filter_map(|file_id| {
                cache
                    .get(file_id)
                    .is_some_and(|state| !state.diagnostics.is_empty())
                    .then_some(*file_id)
            })
            .collect::<FxHashSet<_>>();
        changed_files.extend(by_file.keys().copied());

        for file_id in touched_files {
            let diagnostics = by_file.remove(&file_id).unwrap_or_default();
            let generation =
                cache.get(&file_id).map_or(1, |state| state.generation.saturating_add(1));
            cache.insert(file_id, SemanticDiagnosticState { freshness, generation, diagnostics });
        }

        changed_files
    }
}

#[derive(Debug, Clone, Default)]
struct SemanticDiagnosticState {
    freshness: DiagnosticCommitFreshness,
    generation: u64,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

pub(crate) trait SemanticCompilerCtx {
    fn diagnostic_commit_freshness(&self) -> DiagnosticCommitFreshness;
    fn make_snapshot(&self, cancellation: CancellationToken) -> GlobalStateSnapshot;
    fn spawn_semantic_compiler_task<F>(&mut self, task: F)
    where
        F: FnOnce(crossbeam_channel::Sender<Task>) + Send + 'static;
    fn task_cancel_token(&self) -> CancellationToken;
    fn publish_semantic_diagnostics(&mut self, changed_files: FxHashSet<FileId>);
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
    fn diagnostic_publish_freshness(&self) -> DiagnosticPublishFreshness {
        DiagnosticPublishFreshness::new(
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

    fn spawn_semantic_compiler_task<F>(&mut self, task: F)
    where
        F: FnOnce(crossbeam_channel::Sender<Task>) + Send + 'static,
    {
        self.tasks.task_pool.handle.spawn_and_send_cps(ThreadIntent::Worker, task);
    }

    fn task_cancel_token(&self) -> CancellationToken {
        self.tasks.task_pool.handle.task_token()
    }

    fn publish_semantic_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
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
                        "skipping semantic diagnostics for file without URI: {error:#}"
                    );
                    continue;
                }
            };
            let diagnostics = match snapshot.lsp_diagnostics(file_id) {
                Ok(diagnostics) => diagnostics,
                Err(error) if error.is::<ide::Cancelled>() => {
                    tracing::debug!(?file_id, "semantic diagnostic publish cancelled");
                    continue;
                }
                Err(error) => {
                    tracing::debug!(?file_id, "semantic diagnostic publish failed: {error:#}");
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
    match collect_semantic_diagnostics(&snapshot, profile_ids, &cancellation) {
        Ok(update) => SemanticCompilerTask::Finished { run_id, update },
        Err(err) if err.is::<CancellationError>() => SemanticCompilerTask::Cancelled { run_id },
        Err(err) => SemanticCompilerTask::Failed { run_id, message: err.to_string() },
    }
}

fn collect_semantic_diagnostics(
    snapshot: &GlobalStateSnapshot,
    profile_ids: Vec<CompilationProfileId>,
    cancellation: &CancellationToken,
) -> Result<SemanticCompilerUpdate> {
    let freshness = snapshot.diagnostic_commit_freshness();
    let mut by_file = FxHashMap::default();
    let mut touched_files = FxHashSet::default();

    for profile_id in profile_ids {
        cancellation.check()?;
        touched_files.extend(snapshot.analysis.compilation_profile_file_ids(profile_id)?);
        let diagnostics = snapshot.analysis.compilation_profile_diagnostics(profile_id)?;
        for diagnostic in diagnostics {
            cancellation.check()?;
            if diagnostic.source != ide::diagnostics::DiagnosticSource::SlangSemantic {
                continue;
            }
            let file_id = diagnostic.file_id;
            let line_info = snapshot.line_info(file_id)?;
            let diagnostic = to_proto::diagnostic(snapshot.config.i18n, &line_info, diagnostic);
            by_file.entry(file_id).or_insert_with(Vec::new).push(diagnostic);
        }
    }
    cancellation.check()?;

    Ok(SemanticCompilerUpdate { by_file, touched_files, freshness })
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
