use base_db::{change::Change, source_root::SourceRootConfig};
use crossbeam_channel::{unbounded, Receiver, Sender};
use itertools::Itertools;
use lsp_server::{Message, ReqQueue, Request};
use nohash_hasher::IntMap;
use parking_lot::{RwLock, RwLockUpgradableReadGuard, RwLockWriteGuard};
use project_model::workspace::Workspace;
use rustc_hash::FxHashMap;
use std::time::Instant;
use triomphe::Arc;
use utils::{
    excl_task::ExclTask,
    lines::LineEndings,
    thread::{Pool, ThreadIntent},
};

use crate::{config::Config, main_loop::Task, mem_docs::MemDocs, reload};
use ide::{
    self,
    analysis_host::{Analysis, AnalysisHost},
};
use vfs::{
    self,
    vfs::{ChangedFile, FileId, Vfs},
};

pub(crate) type ReqHandler = fn(&mut GlobalState, lsp_server::Response);

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
            move || sender.send(task()).unwrap()
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

pub(crate) struct GlobalState {
    pub(crate) sender: Sender<Message>,
    pub(crate) req_queue: ReqQueue<(String, Instant), ReqHandler>,

    pub(crate) task_pool: Handle<TaskPool<Task>, Receiver<Task>>,

    pub(crate) config: Arc<Config>,
    pub(crate) source_root_config: SourceRootConfig,

    pub(crate) analysis_host: AnalysisHost,
    pub(crate) mem_docs: MemDocs,

    pub(crate) shutdown_requested: bool,

    pub(crate) vfs_loader: Handle<Box<dyn vfs::loader::Handle>, Receiver<vfs::loader::Message>>,
    pub(crate) vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEndings>)>>,
    pub(crate) vfs_config_version: u32,
    pub(crate) vfs_progress_config_version: u32,
    pub(crate) vfs_progress_n_total: usize,
    pub(crate) vfs_progress_n_done: usize,

    // workspaces
    pub(crate) workspaces: Arc<Vec<Workspace>>,
    pub(crate) fetch_workspaces_task:
        ExclTask<(), Option<(Arc<Vec<Workspace>>, Vec<anyhow::Error>)>>,
}

// immutable
pub(crate) struct GlobalStateSnapshot {
    pub(crate) config: Arc<Config>,
    pub(crate) analysis: Analysis,
    // pub(crate) check_fixes: CheckFixes,
    mem_docs: MemDocs,
    vfs: Arc<RwLock<(Vfs, IntMap<FileId, LineEndings>)>>,
    pub(crate) workspaces: Arc<Vec<Workspace>>,
}

impl std::panic::UnwindSafe for GlobalStateSnapshot {}

impl GlobalState {
    pub(crate) fn new(sender: Sender<lsp_server::Message>, config: Config) -> GlobalState {
        let vfs_loader = {
            let (sender, receiver) = unbounded::<vfs::loader::Message>();
            let handle: vfs_notify::NotifyHandle =
                vfs::loader::Handle::spawn(Box::new(move |msg| sender.send(msg).unwrap()));
            let handle = Box::new(handle) as Box<dyn vfs::loader::Handle>;
            Handle { handle, receiver }
        };

        let task_pool = {
            let (sender, receiver) = unbounded();
            let handle = TaskPool::new_with_threads_num(sender, config.main_loop_threads_num());
            Handle { handle, receiver }
        };

        let mut analysis_host = AnalysisHost::new(None);

        GlobalState {
            sender,
            req_queue: ReqQueue::default(),
            task_pool,
            config: Arc::new(config),
            analysis_host,
            mem_docs: MemDocs::default(),
            shutdown_requested: false,
            source_root_config: SourceRootConfig::default(),

            vfs_loader,
            vfs: Arc::new(RwLock::new((Vfs::default(), IntMap::default()))),
            vfs_config_version: 0,
            vfs_progress_config_version: 0,
            vfs_progress_n_total: 0,
            vfs_progress_n_done: 0,

            workspaces: Arc::new(Vec::new()),
            fetch_workspaces_task: ExclTask::default(),
        }
    }

    pub(crate) fn make_snapshot(&self) -> GlobalStateSnapshot {
        GlobalStateSnapshot {
            config: Arc::clone(&self.config),
            workspaces: Arc::clone(&self.workspaces),
            analysis: self.analysis_host.make_analysis(),
            vfs: Arc::clone(&self.vfs),
            mem_docs: self.mem_docs.clone(),
        }
    }

    pub(crate) fn register_request(&mut self, req_received: Instant, req: &Request) {
        self.req_queue.incoming.register(req.id.clone(), (req.method.clone(), req_received));
    }

    pub(crate) fn is_completed(&self, req: &Request) -> bool {
        self.req_queue.incoming.is_completed(&req.id)
    }
}

// Apply changes
impl GlobalState {
    pub(crate) fn process_changes(&mut self) -> bool {
        let mut write_guard = self.vfs.write();
        let changed_files = write_guard.0.take_changes();
        // downgrade earlier to allow more reader
        let read_guard = RwLockWriteGuard::downgrade_to_upgradable(write_guard);
        let vfs = &read_guard.0;

        // collect changes
        let changed_files = match Self::colease_modifications(changed_files) {
            Some(changed_files) => changed_files,
            None => return false,
        };

        let mut workspace_structure_change = None;
        // A file was added or deleted
        let mut has_structure_changes = false;
        let mut bytes = vec![];
        for changed_file in changed_files {
            let path = vfs.file_path(changed_file.file_id);
            if let Some(path) = path.as_path().map(|apath| apath.to_path_buf()) {
                if changed_file.is_created_or_deleted() {
                    has_structure_changes = true;
                    workspace_structure_change = Some(path);
                } else if reload::should_refresh_for_change(&path, changed_file.change_kind) {
                    workspace_structure_change = Some(path);
                }
            }

            // Collect changes
            let text = if changed_file.exists() {
                let contents = vfs.file_contents(changed_file.file_id).unwrap().to_vec();

                String::from_utf8(contents).ok().map(|text| {
                    // TODO: Consider doing normalization in the `vfs` instead to get rid of some locking?
                    let (text, line_endings) = LineEndings::normalize(text);
                    (Arc::<str>::from(text), line_endings)
                })
            } else {
                None
            };

            bytes.push((changed_file.file_id, text))
        }

        let mut write_guard = RwLockUpgradableReadGuard::upgrade(read_guard);
        let (vfs, line_endings_map) = &mut *write_guard;
        let change = self.collect_changes(bytes, line_endings_map, vfs, has_structure_changes);

        std::mem::drop(write_guard);

        self.analysis_host.apply_change(change);

        if let Some(path) = workspace_structure_change {
            self.fetch_workspaces_task.request(format!("workspace vfs change: {:?}", path), ());
        }

        true
    }

    fn collect_changes(
        &self,
        bytes: Vec<(FileId, Option<(Arc<str>, LineEndings)>)>,
        line_ending_map: &mut IntMap<FileId, LineEndings>,
        vfs: &mut Vfs,
        has_structure_changes: bool,
    ) -> Change {
        let mut change = Change::new();
        for (file_id, text_endings) in bytes {
            match text_endings {
                None => change.add_changed_file(file_id, None),
                Some((text, line_endings)) => {
                    line_ending_map.insert(file_id, line_endings);
                    change.add_changed_file(file_id, Some(text));
                }
            }
        }
        if has_structure_changes {
            let roots = self.source_root_config.partition(vfs);
            change.set_roots(roots);
        }
        change
    }

    fn colease_modifications(vfs_changes: Vec<ChangedFile>) -> Option<Vec<ChangedFile>> {
        if vfs_changes.is_empty() {
            return None;
        }

        // collapse modifications
        use vfs::vfs::ChangeKind::*;

        let mut file_changes = FxHashMap::default();
        for changed_file in vfs_changes {
            file_changes
                .entry(changed_file.file_id)
                .and_modify(|(change, just_created)| {
                    match (change, just_created, changed_file.change_kind) {
                        (change, _, Delete) => *change = Delete,
                        (Create, _, Create | Modify) => {}
                        (Modify, _, Modify) => {}
                        (change @ Delete, just_created, Create) => {
                            *change = Modify;
                            *just_created = true;
                        }
                        (Delete, _, Modify) | (Modify, _, Create) => unreachable!(),
                    }
                })
                .or_insert((changed_file.change_kind, changed_file.change_kind == Create));
        }

        let changed_file = file_changes
            .into_iter()
            .filter(|(_, (kind, just_created))| !(*kind == Delete && *just_created))
            .map(|(file_id, (change_kind, _))| ChangedFile { file_id, change_kind })
            .collect_vec();

        Some(changed_file)
    }
}
