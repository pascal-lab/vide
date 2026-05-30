mod diagnostics;
mod dispatcher;
mod handlers;
pub mod main_loop;
mod mem_docs;
pub(crate) mod process_changes;
mod project_status;
mod qihe;
pub mod reload;
pub mod respond;
mod response_effect;
pub(crate) mod snapshot;
mod trace;
mod workspace_state;

use std::{
    collections::HashMap,
    panic::{self, AssertUnwindSafe},
    time::Instant,
};

use crossbeam_channel::{Receiver, Sender, unbounded};
use hir::base_db::{
    project::{ProjectConfig, SharedProjectConfig},
    salsa::Durability,
    source_db::SourceDb,
    source_root::SourceRootConfig,
};
use ide::analysis_host::AnalysisHost;
use lsp_server::{Message, ReqQueue, Request};
use lsp_types::{TraceValue, Url};
use nohash_hasher::IntMap;
use parking_lot::{Mutex, RwLock};
use project_model::Workspace;
use rustc_hash::{FxHashMap, FxHashSet};
use triomphe::Arc;
use utils::{
    cancellation::CancellationToken,
    excl_task::ExclTask,
    lines::LineEnding,
    thread::{Pool, ThreadIntent},
};
use vfs::{self, FileId, Vfs, notify::NotifyHandle};

#[cfg(test)]
pub(crate) use self::workspace_state::VfsProgress;
pub(crate) use self::workspace_state::{
    WorkspaceFetchCause, WorkspaceFetchCompletion, WorkspaceGeneration,
};
use self::{
    diagnostics::{DiagnosticCommitFreshness, DiagnosticFileRevision, DiagnosticPublishFreshness},
    main_loop::{DiagnosticPublishKey, Task},
    mem_docs::MemDocs,
    snapshot::GlobalStateSnapshot,
    trace::LspTrace,
    workspace_state::WorkspaceVfsReadiness,
};
use crate::config::{Config, ConfigError};

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

pub(crate) struct Handle<H, C> {
    pub(crate) handle: H,
    pub(crate) receiver: C,
}

pub(crate) type ReqHandler = fn(&mut GlobalState, lsp_server::Response);
pub(crate) const DEFAULT_REQ_HANDLER: ReqHandler = |_, _| {};

pub(crate) struct GlobalState {
    pub(crate) sender: Sender<Message>,
    pub(crate) lsp_trace: LspTrace,

    pub(crate) req_queue: ReqQueue<(String, Instant), ReqHandler>,

    pub(crate) task_pool: Handle<TaskPool<Task>, Receiver<Task>>,

    pub(crate) config: Arc<Config>,
    pub(crate) config_errors: Option<ConfigError>,
    pub(crate) source_root_config: SourceRootConfig,
    pub(crate) project_config: SharedProjectConfig,

    pub(crate) analysis_host: AnalysisHost,
    pub(crate) mem_docs: MemDocs,

    pub(crate) shutdown_requested: bool,

    pub(crate) semantic_tokens_cache: Arc<Mutex<FxHashMap<Url, lsp_types::SemanticTokens>>>,
    pub(crate) published_diagnostics: FxHashMap<DiagnosticPublishKey, Vec<lsp_types::Diagnostic>>,
    pub(crate) pending_diagnostic_requests: Vec<Request>,
    // didOpen/didClose can change the URI set for a file without changing its
    // text. Keep those target changes explicit so push diagnostics converge at
    // the normal change-processing boundary.
    pub(crate) pending_document_diagnostic_targets: FxHashSet<FileId>,
    pub(crate) diagnostics_revision: u64,
    pub(crate) diagnostic_target_revision: u64,
    pub(crate) diagnostic_file_revisions: FxHashMap<FileId, DiagnosticFileRevision>,
    pub(crate) qihe_diagnostics: Arc<Mutex<FxHashMap<FileId, QiheDiagnosticState>>>,
    // Only the latest Qihe run is allowed to commit diagnostics or logs.
    pub(crate) qihe_run_generation: qihe::QiheRunId,
    pub(crate) qihe_active_progress_token: Option<String>,
    pub(crate) qihe_active_cancel_token: Option<CancellationToken>,

    pub(crate) vfs_loader: Handle<Box<dyn vfs::loader::Handle>, Receiver<vfs::loader::Message>>,
    pub(crate) vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEnding>)>>,
    pub(crate) workspace_vfs: WorkspaceVfsReadiness,

    // workspaces
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

        GlobalState {
            sender,
            lsp_trace: LspTrace::new(initial_trace),
            req_queue: ReqQueue::default(),
            task_pool,
            config: Arc::new(config),
            config_errors: None,
            analysis_host,
            mem_docs: MemDocs::default(),
            shutdown_requested: false,
            source_root_config: SourceRootConfig::default(),
            project_config: Arc::new(ProjectConfig::default()),

            semantic_tokens_cache: Arc::new(Default::default()),
            published_diagnostics: FxHashMap::default(),
            pending_diagnostic_requests: Vec::new(),
            pending_document_diagnostic_targets: FxHashSet::default(),
            diagnostics_revision: 0,
            diagnostic_target_revision: 0,
            diagnostic_file_revisions: FxHashMap::default(),
            qihe_diagnostics: Arc::new(Mutex::new(FxHashMap::default())),
            qihe_run_generation: qihe::QiheRunId::default(),
            qihe_active_progress_token: None,
            qihe_active_cancel_token: None,

            vfs_loader,
            vfs: Arc::new(RwLock::new((Vfs::default(), IntMap::default()))),
            workspace_vfs: WorkspaceVfsReadiness::default(),

            workspaces: Arc::from(vec![]),
            fetch_workspaces_task: ExclTask::default(),
            registered_client_file_watcher_globs: None,
        }
    }

    pub(crate) fn make_snapshot(&self) -> GlobalStateSnapshot {
        self.make_snapshot_with_cancel(self.task_pool.handle.task_token())
    }

    pub(crate) fn make_snapshot_with_cancel(
        &self,
        cancellation: CancellationToken,
    ) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            config: Arc::clone(&self.config),
            workspaces: Arc::clone(&self.workspaces),
            analysis: self.analysis_host.make_analysis(),
            vfs: Arc::clone(&self.vfs),
            mem_docs: self.mem_docs.clone(),
            sema_tokens_cache: Arc::clone(&self.semantic_tokens_cache),
            qihe_diagnostics: Arc::clone(&self.qihe_diagnostics),
            diagnostic_publish_freshness: self.diagnostic_publish_freshness(),
            diagnostic_file_revisions: self.diagnostic_file_revisions.clone(),
            cancellation,
            accepted_response_effects: Default::default(),
        }
    }

    pub(crate) fn diagnostic_publish_freshness(&self) -> DiagnosticPublishFreshness {
        DiagnosticPublishFreshness::new(
            self.diagnostics_revision,
            self.diagnostic_target_revision,
            self.workspace_vfs.diagnostic_readiness_revision(),
        )
    }

    pub(crate) fn diagnostic_commit_freshness(&self) -> DiagnosticCommitFreshness {
        self.diagnostic_publish_freshness().commit()
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
        self.req_queue.incoming.register(req.id.clone(), (req.method.clone(), req_received));
        self.task_pool.handle.register_request(req.id.clone());
    }

    pub(crate) fn is_completed(&self, req: &Request) -> bool {
        self.req_queue.incoming.is_completed(&req.id)
    }

    pub(crate) fn cancel(&mut self, req_id: lsp_server::RequestId) {
        self.task_pool.handle.cancel_request(&req_id);
        if let Some(response) = self.req_queue.incoming.cancel(req_id) {
            self.task_pool.handle.complete_request(&response.id);
            self.send(response.into());
        }
    }

    pub(crate) fn cancel_all_tasks(&mut self) {
        self.task_pool.handle.cancel_all();
    }
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
