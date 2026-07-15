mod diagnostics;
mod dispatch;
mod dispatcher;
pub(crate) mod event_loop;
mod handlers;
pub mod main_loop;
mod mem_docs;
pub(crate) mod process_changes;
mod project_status;
mod qihe;
pub mod reload;
pub mod respond;
mod response_effect;
mod semantic_compiler;
pub(crate) mod snapshot;
pub(crate) mod task;
mod trace;
mod workspace_state;

use std::{sync::Arc as StdArc, time::Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use hir::base_db::{
    project::{CompilationProfileId, ProjectConfig, SharedProjectConfig},
    salsa::Durability,
    source_db::SourceDb,
    source_root::SourceRootConfig,
};
use ide::analysis_host::AnalysisHost;
use lsp_server::{Message, ReqQueue, Request};
use lsp_types::{NumberOrString, TraceValue, Url};
use nohash_hasher::IntMap;
use parking_lot::{Mutex, RwLock};
use project_model::Workspace;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use utils::{cancellation::CancellationToken, excl_task::ExclTask, lines::LineEnding};
use vfs::{self, FileId, Vfs, notify::NotifyHandle};

#[cfg(test)]
pub(crate) use self::workspace_state::VfsProgress;
pub(crate) use self::workspace_state::{
    WorkspaceFetchCause, WorkspaceFetchCompletion, WorkspaceGeneration,
};
use self::{
    diagnostics::{
        DiagnosticCommitFreshness, DiagnosticFileRevision, DiagnosticPublishFreshness,
        DiagnosticSource, publisher::DiagnosticPublishKey,
    },
    mem_docs::MemDocs,
    snapshot::GlobalStateSnapshot,
    task::{QiheTask, SemanticCompilerTask, Task, TaskPool},
    trace::LspTrace,
    workspace_state::WorkspaceVfsReadiness,
};
use crate::{
    config::{Config, ConfigError},
    lsp_ext::ext::RunQiheAnalysisParams,
};

pub(crate) struct Handle<H, C> {
    pub(crate) handle: H,
    pub(crate) receiver: C,
}

pub(crate) type ReqHandler = fn(&mut GlobalState, lsp_server::Response);
pub(crate) const DEFAULT_REQ_HANDLER: ReqHandler = |_, _| {};

pub(crate) struct GlobalState {
    pub(crate) client: ClientState,
    pub(crate) config_state: ConfigState,
    pub(crate) analysis: AnalysisState,
    pub(crate) diagnostics: DiagnosticsState,
    pub(crate) workspace: WorkspaceState,
    pub(crate) qihe: qihe::Qihe,
    pub(crate) semantic_compiler: semantic_compiler::SemanticCompiler,
    pub(crate) external_sources: Vec<StdArc<dyn DiagnosticSource>>,
    pub(crate) tasks: TaskState,
}

pub(crate) struct ClientState {
    pub(crate) sender: Sender<Message>,
    pub(crate) lsp_trace: LspTrace,
    pub(crate) req_queue: ReqQueue<(String, Instant), ReqHandler>,
    pub(crate) shutdown_requested: bool,
}

pub(crate) struct TaskState {
    pub(crate) task_pool: Handle<TaskPool<Task>, Receiver<Task>>,
}

pub(crate) struct ConfigState {
    pub(crate) config: Arc<Config>,
    pub(crate) config_errors: Option<ConfigError>,
    pub(crate) source_root_config: SourceRootConfig,
    pub(crate) project_config: SharedProjectConfig,
}

pub(crate) struct AnalysisState {
    pub(crate) analysis_host: AnalysisHost,
    pub(crate) mem_docs: MemDocs,
    pub(crate) semantic_tokens_cache: Arc<Mutex<FxHashMap<Url, lsp_types::SemanticTokens>>>,
}

pub(crate) struct DiagnosticsState {
    pub(crate) published_diagnostics: FxHashMap<DiagnosticPublishKey, Vec<lsp_types::Diagnostic>>,
    // didOpen/didClose can change the URI set for a file without changing its
    // text. Keep those target changes explicit so push diagnostics converge at
    // the normal change-processing boundary.
    pub(crate) pending_document_diagnostic_targets: FxHashSet<FileId>,
    pub(crate) diagnostics_revision: u64,
    pub(crate) diagnostic_target_revision: u64,
    pub(crate) diagnostic_file_revisions: FxHashMap<FileId, DiagnosticFileRevision>,
}

pub(crate) struct WorkspaceState {
    pub(crate) vfs_loader: Handle<Box<dyn vfs::loader::Handle>, Receiver<vfs::loader::Message>>,
    pub(crate) vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEnding>)>>,
    pub(crate) workspace_vfs: WorkspaceVfsReadiness,
    pub(crate) workspaces: Arc<Vec<Workspace>>,
    pub(crate) fetch_workspaces_task:
        ExclTask<(Arc<Vec<Workspace>>, Vec<anyhow::Error>), WorkspaceFetchCause>,
    pub(crate) registered_client_file_watcher_globs: Option<Vec<String>>,
}

impl GlobalState {
    pub(crate) fn new(
        sender: Sender<lsp_server::Message>,
        config: Config,
        initial_trace: TraceValue,
    ) -> GlobalState {
        let vfs_loader = {
            let (sender, receiver) = unbounded::<vfs::loader::Message>();
            let handle: NotifyHandle = vfs::loader::Handle::spawn(sender);
            let handle = Box::new(handle) as Box<dyn vfs::loader::Handle>;
            Handle { handle, receiver }
        };

        let task_pool = {
            let (sender, receiver) = unbounded();
            let handle = TaskPool::new_with_threads_num(sender, config.main_loop_threads_num());
            Handle { handle, receiver }
        };

        let mut analysis_host = AnalysisHost::new(None);
        let diagnostics_config = Arc::new(config.diagnostics_config());
        analysis_host
            .raw_db_mut()
            .set_diagnostics_config_with_durability(diagnostics_config, Durability::HIGH);

        let qihe_diagnostics = qihe::QiheDiagnostics::new();
        let qihe = qihe::Qihe::new(qihe_diagnostics);
        let qihe_source: StdArc<dyn DiagnosticSource> = StdArc::new(qihe.diagnostics_snapshot());
        let semantic_diagnostics = semantic_compiler::SemanticDiagnostics::new();
        let semantic_compiler = semantic_compiler::SemanticCompiler::new(semantic_diagnostics);
        let semantic_source: StdArc<dyn DiagnosticSource> =
            StdArc::new(semantic_compiler.diagnostics_snapshot());
        let external_sources = vec![qihe_source, semantic_source];

        GlobalState {
            client: ClientState {
                sender,
                lsp_trace: LspTrace::new(initial_trace),
                req_queue: ReqQueue::default(),
                shutdown_requested: false,
            },
            config_state: ConfigState {
                config: Arc::new(config),
                config_errors: None,
                source_root_config: SourceRootConfig::default(),
                project_config: Arc::new(ProjectConfig::default()),
            },
            analysis: AnalysisState {
                analysis_host,
                mem_docs: MemDocs::default(),
                semantic_tokens_cache: Arc::new(Default::default()),
            },
            diagnostics: DiagnosticsState {
                published_diagnostics: FxHashMap::default(),
                pending_document_diagnostic_targets: FxHashSet::default(),
                diagnostics_revision: 0,
                diagnostic_target_revision: 0,
                diagnostic_file_revisions: FxHashMap::default(),
            },
            workspace: WorkspaceState {
                vfs_loader,
                vfs: Arc::new(RwLock::new((Vfs::default(), IntMap::default()))),
                workspace_vfs: WorkspaceVfsReadiness::default(),
                workspaces: Arc::from(vec![]),
                fetch_workspaces_task: ExclTask::default(),
                registered_client_file_watcher_globs: None,
            },
            qihe,
            semantic_compiler,
            external_sources,
            tasks: TaskState { task_pool },
        }
    }

    pub(crate) fn make_snapshot(&self) -> GlobalStateSnapshot {
        self.make_snapshot_with_cancel(self.tasks.task_pool.handle.task_token())
    }

    pub(crate) fn make_snapshot_with_cancel(
        &self,
        cancellation: CancellationToken,
    ) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            config: Arc::clone(&self.config_state.config),
            workspaces: Arc::clone(&self.workspace.workspaces),
            analysis: self.analysis.analysis_host.make_analysis(),
            vfs: Arc::clone(&self.workspace.vfs),
            mem_docs: self.analysis.mem_docs.clone(),
            sema_tokens_cache: Arc::clone(&self.analysis.semantic_tokens_cache),
            external_sources: self.external_sources.clone(),
            diagnostic_publish_freshness: self.diagnostic_publish_freshness(),
            diagnostic_file_revisions: self.diagnostics.diagnostic_file_revisions.clone(),
            cancellation,
            accepted_response_effects: Default::default(),
        }
    }

    pub(crate) fn diagnostic_publish_freshness(&self) -> DiagnosticPublishFreshness {
        DiagnosticPublishFreshness::new(
            self.diagnostics.diagnostics_revision,
            self.diagnostics.diagnostic_target_revision,
            self.workspace.workspace_vfs.diagnostic_readiness_revision(),
        )
    }

    pub(crate) fn spawn_qihe_analysis(&mut self, params: RunQiheAnalysisParams) {
        qihe::with_global_ctx(self, |qihe, ctx| qihe.start(params, ctx));
    }

    pub(crate) fn handle_qihe_task(&mut self, task: QiheTask) {
        qihe::with_global_ctx(self, |qihe, ctx| qihe.handle(task, ctx));
    }

    pub(crate) fn schedule_semantic_compiler(&mut self, profile_ids: Vec<CompilationProfileId>) {
        semantic_compiler::with_global_ctx(self, |semantic_compiler, ctx| {
            semantic_compiler.schedule(profile_ids, ctx)
        });
    }

    pub(crate) fn handle_semantic_compiler_task(&mut self, task: SemanticCompilerTask) {
        semantic_compiler::with_global_ctx(self, |semantic_compiler, ctx| {
            semantic_compiler.handle(task, ctx)
        });
    }

    pub(crate) fn cancel_work_done_progress(
        &mut self,
        params: lsp_types::WorkDoneProgressCancelParams,
    ) {
        let token = match params.token {
            NumberOrString::String(token) => token,
            NumberOrString::Number(token) => token.to_string(),
        };
        self.qihe.cancel_progress_token(&token);
    }

    #[cfg(test)]
    pub(crate) fn publish_qihe_diagnostics(&mut self, changed_files: FxHashSet<FileId>) {
        qihe::with_global_ctx(self, |qihe, ctx| qihe.publish_diagnostics(changed_files, ctx));
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct QiheDiagnosticState {
    pub(crate) freshness: DiagnosticCommitFreshness,
    pub(crate) generation: u64,
    pub(crate) diagnostics: Vec<lsp_types::Diagnostic>,
}

// handle request
impl GlobalState {
    pub(crate) fn register_request(&mut self, req_received: Instant, req: &Request) {
        self.client.req_queue.incoming.register(req.id.clone(), (req.method.clone(), req_received));
        self.tasks.task_pool.handle.register_request(req.id.clone());
    }

    pub(crate) fn is_completed(&self, req: &Request) -> bool {
        self.client.req_queue.incoming.is_completed(&req.id)
    }

    pub(crate) fn cancel(&mut self, req_id: lsp_server::RequestId) {
        self.tasks.task_pool.handle.cancel_request(&req_id);
        if let Some(response) = self.client.req_queue.incoming.cancel(req_id) {
            self.tasks.task_pool.handle.complete_request(&response.id);
            self.send(response.into());
        }
    }

    pub(crate) fn cancel_all_tasks(&mut self) {
        self.tasks.task_pool.handle.cancel_all();
    }
}
