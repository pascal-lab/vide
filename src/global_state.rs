mod dispatcher;
mod handlers;
pub mod main_loop;
mod mem_docs;
pub(crate) mod process_changes;
mod project_status;
mod qihe;
pub mod reload;
pub mod respond;
pub(crate) mod snapshot;
mod trace;

use std::time::Instant;

use base_db::{
    project::{ProjectConfig, SharedProjectConfig},
    source_db::SourceDb,
    source_root::SourceRootConfig,
};
use crossbeam_channel::{Receiver, Sender, unbounded};
use ide::analysis_host::AnalysisHost;
use lsp_server::{Message, ReqQueue, Request};
use lsp_types::{TraceValue, Url};
use nohash_hasher::IntMap;
use parking_lot::{Mutex, RwLock};
use project_model::Workspace;
use rustc_hash::FxHashMap;
use triomphe::Arc;
use utils::{
    excl_task::ExclTask,
    lines::LineEnding,
    thread::{Pool, ThreadIntent},
};
use vfs::{self, FileId, Vfs};

use self::{main_loop::Task, mem_docs::MemDocs, snapshot::GlobalStateSnapshot, trace::LspTrace};
use crate::config::{Config, ConfigError};

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
            move || {
                if sender.send(task()).is_err() {
                    tracing::debug!("task result dropped because main loop receiver is closed");
                }
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
            move || task(sender)
        })
    }
}

pub(crate) struct Handle<H, C> {
    pub(crate) handle: H,
    pub(crate) receiver: C,
}

#[derive(Default)]
pub(crate) struct VfsProgress {
    pub(crate) config_version: u32,
    pub(crate) n_done: usize,
    pub(crate) n_total: usize,
}

impl VfsProgress {
    fn in_progress(&self) -> bool {
        self.n_done < self.n_total
    }
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
    pub(crate) diagnostics: FxHashMap<FileId, Vec<lsp_types::Diagnostic>>,
    pub(crate) qihe_diagnostics: Arc<Mutex<FxHashMap<FileId, QiheDiagnosticState>>>,

    pub(crate) vfs_loader: Handle<Box<dyn vfs::loader::Handle>, Receiver<vfs::loader::Message>>,
    pub(crate) vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEnding>)>>,
    pub(crate) vfs_config_version: u32,
    pub(crate) vfs_progress: VfsProgress,

    // workspaces
    pub(crate) workspaces: Arc<Vec<Workspace>>,
    pub(crate) fetch_workspaces_task: ExclTask<(Arc<Vec<Workspace>>, Vec<anyhow::Error>)>,
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
            let handle: vfs_notify::NotifyHandle = vfs::loader::Handle::spawn(sender);
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
        analysis_host.raw_db_mut().set_diagnostics_config_with_durability(
            diagnostics_config,
            base_db::salsa::Durability::HIGH,
        );

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
            diagnostics: FxHashMap::default(),
            qihe_diagnostics: Arc::new(Mutex::new(FxHashMap::default())),

            vfs_loader,
            vfs: Arc::new(RwLock::new((Vfs::default(), IntMap::default()))),
            vfs_config_version: 0,
            vfs_progress: VfsProgress::default(),

            workspaces: Arc::from(vec![]),
            fetch_workspaces_task: ExclTask::default(),
            registered_client_file_watcher_globs: None,
        }
    }

    pub(crate) fn make_snapshot(&self) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            config: Arc::clone(&self.config),
            workspaces: Arc::clone(&self.workspaces),
            analysis: self.analysis_host.make_analysis(),
            vfs: Arc::clone(&self.vfs),
            mem_docs: self.mem_docs.clone(),
            sema_tokens_cache: Arc::clone(&self.semantic_tokens_cache),
            qihe_diagnostics: Arc::clone(&self.qihe_diagnostics),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct QiheDiagnosticState {
    pub(crate) generation: u64,
    pub(crate) diagnostics: Vec<lsp_types::Diagnostic>,
}

// handle request
impl GlobalState {
    pub(crate) fn register_request(&mut self, req_received: Instant, req: &Request) {
        self.req_queue.incoming.register(req.id.clone(), (req.method.clone(), req_received));
    }

    pub(crate) fn is_completed(&self, req: &Request) -> bool {
        self.req_queue.incoming.is_completed(&req.id)
    }

    pub(crate) fn cancel(&mut self, req_id: lsp_server::RequestId) {
        if let Some(response) = self.req_queue.incoming.cancel(req_id) {
            self.send(response.into());
        }
    }
}
