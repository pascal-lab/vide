//! Resident slang elaboration service (T4c).
//!
//! This is a backend worker, not a cache. Slang's `Compilation` is not
//! incremental, so the value does not belong in salsa or in
//! [`crate::incrementality::ProductStore`]. One worker thread owns the live
//! compilations; queries name a snapshot revision and get a typed result.
//!
//! [`ElabResult`] is the T7 safety rope: callers can tell "slang said nothing"
//! (`Ready(None)`) from "this snapshot is gone" (`Stale`) from "the worker
//! could not answer" (`Unavailable`). Silent `None` is a bug.
//!
//! Every query is the same three steps — reach the live compilation for a
//! revision, run one closure on it, ship the answer back. That shape is
//! written once in [`Worker::query`] and [`ElaborationService::query`]; the
//! public methods only name a slang entry point.

use std::{
    fmt,
    panic::{self, AssertUnwindSafe},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use base_db::{
    Cancelled, analysis_snapshot::AnalysisSnapshotId, project::CompilationProfileId,
    source_db::SourceRootDb,
};
use preproc_expand::compilation_plan::{
    self, CompilationRootKind, compilation_source_buffers_for_plan,
};
use rustc_hash::FxHashMap;
use slang_sys::compilation::{Compilation, HierInstance, MemberInfo, SymbolInfo};
use syntax::{SyntaxTreeBuffer, SyntaxTreeOptions};

use crate::db::root_db::RootDb;

/// How long a request-path query waits before giving up on the worker.
///
/// A cold snapshot needs a full slang elaboration, which is far longer than
/// this. Waiting it out on the keyboard path is a hang, not a degradation.
/// The build is not cancelled by giving up: [`AnalysisHost`] prewarms it off
/// the request path, and a later query for the same revision finds it ready.
///
/// [`AnalysisHost`]: crate::analysis_host::AnalysisHost
const INTERACTIVE_TIMEOUT: Duration = Duration::from_millis(150);

const KEPT_GENERATIONS: usize = 2;

/// How long a caller is willing to wait for the worker.
#[derive(Debug, Clone, Copy)]
enum Wait {
    /// Request path. Never block the editor; degrade to HIR instead.
    Interactive,
    /// Prewarm and tests: the answer matters, the latency does not.
    UntilDone,
}

/// Snapshot tag carried by every query. Matches [`AnalysisSnapshotId`].
pub type ElabRevision = AnalysisSnapshotId;

/// Why the resident compilation could not answer. Each arm implies a
/// different caller action, so they must not be collapsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    /// The wait elapsed while the worker was still compiling this snapshot.
    /// The build continues; a later query for the same revision can be
    /// `Ready`.
    NotReady,
    /// The file belongs to no compilation profile, so no elaboration covers
    /// it. Waiting does not help; the workspace configuration has to change.
    OutsideAnyProfile,
    /// Salsa cancelled the rebuild because the workspace moved on. The next
    /// revision will build.
    Cancelled,
    /// Slang unwound while answering. The payload names the query.
    Crashed(String),
    /// The worker thread is gone.
    WorkerGone,
}

/// Answer from the resident compilation. The three arms are the contract:
/// empty, stale, and unavailable are not the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabResult<T> {
    Ready(Option<T>),
    Stale { have: ElabRevision, want: ElabRevision },
    Unavailable(UnavailableReason),
}

impl<T> ElabResult<T> {
    /// The answer, recording the degradation when there is not one.
    ///
    /// `Ready(None)` is an answer: slang elaborated and found nothing there,
    /// so absence is the truth. Every other arm means slang did *not*
    /// answer, which is a different fact, and callers that fall back to HIR
    /// must not make it indistinguishable from absence. Routing the whole
    /// enum through here is what keeps the fallback visible.
    ///
    /// `feature` names the caller; the slang entry point is already in
    /// [`UnavailableReason::Crashed`].
    pub fn answered(self, feature: &'static str) -> Option<T> {
        match self {
            ElabResult::Ready(answer) => answer,
            // Routine while typing: the snapshot rolled, or the build for
            // this one is still running.
            ElabResult::Stale { have, want } => {
                tracing::debug!(feature, ?have, ?want, "elaboration is behind; HIR answers");
                None
            }
            ElabResult::Unavailable(
                reason @ (UnavailableReason::NotReady
                | UnavailableReason::OutsideAnyProfile
                | UnavailableReason::Cancelled),
            ) => {
                tracing::debug!(feature, ?reason, "elaboration declined; HIR answers");
                None
            }
            // Not routine. Fidelity is gone until someone looks at this.
            ElabResult::Unavailable(reason) => {
                tracing::warn!(feature, ?reason, "elaboration failed; HIR answers");
                None
            }
        }
    }
}

/// The payload-free half of [`ElabResult`]. Reaching the live compilation can
/// fail before a query type is even involved, so that step returns this.
#[derive(Debug, Clone, PartialEq, Eq)]
enum NotAnswered {
    Stale { have: ElabRevision, want: ElabRevision },
    Unavailable(UnavailableReason),
}

impl NotAnswered {
    fn into_result<T>(self) -> ElabResult<T> {
        match self {
            NotAnswered::Stale { have, want } => ElabResult::Stale { have, want },
            NotAnswered::Unavailable(reason) => ElabResult::Unavailable(reason),
        }
    }
}

#[derive(Clone)]
pub struct ElaborationService {
    tx: Sender<Job>,
}

impl fmt::Debug for ElaborationService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElaborationService").finish()
    }
}

/// One unit of worker work. Every query is a closure so that the reply type
/// stays with the caller instead of becoming another channel variant.
enum Job {
    Run(Box<dyn FnOnce(&mut Worker) + Send>),
    Shutdown,
}

struct Generation {
    revision: ElabRevision,
    profiles: FxHashMap<Option<CompilationProfileId>, Compilation>,
}

impl ElaborationService {
    pub fn spawn() -> (Self, JoinHandle<()>) {
        let (tx, rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("vide-elaboration".to_owned())
            .spawn(move || worker_loop(rx))
            .expect("failed to spawn elaboration worker");
        (Self { tx }, worker)
    }

    /// Hand one job to the worker and wait for its answer.
    ///
    /// This is the only place that talks to the channel, so timeout and
    /// disconnect are classified once.
    fn dispatch<T: Send + 'static>(
        &self,
        wait: Wait,
        job: impl FnOnce(&mut Worker) -> ElabResult<T> + Send + 'static,
    ) -> ElabResult<T> {
        let (reply_tx, reply_rx) = mpsc::channel();
        let run = move |worker: &mut Worker| {
            let _ = reply_tx.send(job(worker));
        };
        if self.tx.send(Job::Run(Box::new(run))).is_err() {
            return ElabResult::Unavailable(UnavailableReason::WorkerGone);
        }
        let received = match wait {
            Wait::Interactive => reply_rx.recv_timeout(INTERACTIVE_TIMEOUT).map_err(|err| match err
            {
                RecvTimeoutError::Timeout => UnavailableReason::NotReady,
                RecvTimeoutError::Disconnected => UnavailableReason::WorkerGone,
            }),
            Wait::UntilDone => reply_rx.recv().map_err(|_| UnavailableReason::WorkerGone),
        };
        received.unwrap_or_else(ElabResult::Unavailable)
    }

    /// Run one slang entry point on the live compilation for `revision`.
    ///
    /// `what` names the query in [`UnavailableReason::Crashed`]. `run`
    /// executes on the worker thread, so the compilation never crosses a
    /// thread boundary.
    fn query<T: Send + 'static>(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        what: &'static str,
        run: impl FnOnce(&mut Compilation) -> Option<T> + Send + 'static,
    ) -> ElabResult<T> {
        let db = db.clone();
        self.dispatch(Wait::Interactive, move |worker| {
            worker.query(&db, revision, profile, what, run)
        })
    }

    /// Build this snapshot's compilations, waiting for slang to finish.
    ///
    /// The revision prewarm calls this so that the request path finds the
    /// answer ready instead of paying for a cold elaboration on the keyboard
    /// path. Blocking here is the point: this is not the request path.
    pub fn prewarm(&self, db: &RootDb, revision: ElabRevision) -> ElabResult<()> {
        let db = db.clone();
        self.dispatch(Wait::UntilDone, move |worker| match worker.generation(&db, revision) {
            Ok(_) => ElabResult::Ready(Some(())),
            Err(not_answered) => not_answered.into_result(),
        })
    }

    pub fn lookup_symbol(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        path: &str,
        offset: usize,
    ) -> ElabResult<SymbolInfo> {
        let path = path.to_owned();
        self.query(db, revision, profile, "symbol", move |slang| {
            slang.lookup_symbol(&path, offset)
        })
    }

    pub fn lookup_scoped(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        left: &str,
        right: &str,
    ) -> ElabResult<SymbolInfo> {
        let (left, right) = (left.to_owned(), right.to_owned());
        self.query(db, revision, profile, "scoped", move |slang| {
            slang.lookup_scoped(&left, &right)
        })
    }

    pub fn list_scope_members(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        name: &str,
    ) -> ElabResult<Vec<MemberInfo>> {
        let name = name.to_owned();
        self.query(db, revision, profile, "scope members", move |slang| {
            Some(slang.list_scope_members(&name))
        })
    }

    pub fn list_members(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        path: &str,
        offset: usize,
    ) -> ElabResult<Vec<MemberInfo>> {
        let path = path.to_owned();
        self.query(db, revision, profile, "members", move |slang| {
            Some(slang.list_members(&path, offset))
        })
    }

    pub fn lookup_type(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        path: &str,
        start: usize,
        end: usize,
    ) -> ElabResult<String> {
        let path = path.to_owned();
        self.query(db, revision, profile, "type", move |slang| {
            slang.lookup_type(&path, start, end)
        })
    }

    pub fn list_instances(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
    ) -> ElabResult<Vec<HierInstance>> {
        self.query(db, revision, profile, "instances", move |slang| {
            Some(slang.list_instances())
        })
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Job::Shutdown);
    }

}

fn worker_loop(rx: Receiver<Job>) {
    let mut worker = Worker::default();
    while let Ok(job) = rx.recv() {
        match job {
            Job::Run(run) => run(&mut worker),
            Job::Shutdown => break,
        }
    }
}

/// The live compilations. Owned by one thread; never shared.
#[derive(Default)]
struct Worker {
    /// Newest last. At most [`KEPT_GENERATIONS`] entries.
    generations: Vec<Generation>,
}

impl Worker {
    fn query<T>(
        &mut self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        what: &'static str,
        run: impl FnOnce(&mut Compilation) -> Option<T>,
    ) -> ElabResult<T> {
        let slang = match self.compilation(db, revision, profile) {
            Ok(slang) => slang,
            Err(not_answered) => return not_answered.into_result(),
        };
        // Slang is a foreign library reached over FFI. An unwind out of it is
        // its failure, not a broken invariant of ours, and it must not take
        // the worker down with it. Rust-side bugs are not caught here: they
        // live in `rebuild`, which propagates.
        match panic::catch_unwind(AssertUnwindSafe(|| run(slang))) {
            Ok(answer) => ElabResult::Ready(answer),
            Err(_) => ElabResult::Unavailable(UnavailableReason::Crashed(format!(
                "slang unwound during {what} lookup"
            ))),
        }
    }

    /// The live compilation for one snapshot and profile, building the
    /// snapshot first if it is newer than everything kept.
    fn compilation(
        &mut self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
    ) -> Result<&mut Compilation, NotAnswered> {
        let index = self.generation(db, revision)?;
        self.generations[index]
            .profiles
            .get_mut(&profile)
            .ok_or(NotAnswered::Unavailable(UnavailableReason::OutsideAnyProfile))
    }

    fn generation(
        &mut self,
        db: &RootDb,
        revision: ElabRevision,
    ) -> Result<usize, NotAnswered> {
        if let Some(index) = self.generations.iter().position(|slot| slot.revision == revision) {
            return Ok(index);
        }
        if let Some(newest) = self.generations.last()
            && revision < newest.revision
        {
            return Err(NotAnswered::Stale { have: newest.revision, want: revision });
        }
        // `Cancelled::catch` unwinds again for anything that is not salsa
        // cancellation, so a Rust bug in the rebuild kills this worker and
        // every later query reports `WorkerGone`. That is louder than a
        // swallowed panic and does not poison the revision.
        let generation = Cancelled::catch(|| rebuild(db, revision))
            .map_err(|_| NotAnswered::Unavailable(UnavailableReason::Cancelled))?;
        self.generations.push(generation);
        if self.generations.len() > KEPT_GENERATIONS {
            self.generations.remove(0);
        }
        Ok(self.generations.len() - 1)
    }
}

/// Build every profile's compilation for one snapshot.
///
/// Every root is parsed fresh. Carrying a `SyntaxTree` over from the previous
/// generation is not possible as the FFI stands: a `Compilation` owns a
/// `SourceSession`, every tree belongs to the session it was parsed in, and
/// `add_syntax_tree` rejects a foreign one. Reusing trees needs a session
/// that outlives a single generation, with `SourceManager::replaceBuffer` for
/// the edited files — a change in `slang-sys`, not here. Do not reintroduce
/// per-root reuse without it; it aborts the process.
fn rebuild(db: &RootDb, revision: ElabRevision) -> Generation {
    // A workspace with no configured profile still compiles: the plan for
    // `None` covers every root. This is the unconfigured case, not the
    // orphan-file bucket that profile partitioning has to avoid.
    let ids = db.project_config().profile_ids();
    let profile_ids: Vec<Option<CompilationProfileId>> =
        if ids.is_empty() { vec![None] } else { ids.into_iter().map(Some).collect() };

    let profiles = profile_ids
        .into_iter()
        .map(|profile_id| (profile_id, compile_profile(db, profile_id)))
        .collect();
    Generation { revision, profiles }
}

fn compile_profile(db: &RootDb, profile_id: Option<CompilationProfileId>) -> Compilation {
    let plan = db.compilation_plan_for_profile(profile_id);
    let context = db.compilation_context(profile_id);
    let include_paths: Vec<String> =
        context.include_dirs.iter().map(ToString::to_string).collect();

    let mut compilation = Compilation::new_with_top_modules(&context.top_modules);
    compilation.register_source_buffers(
        &compilation_source_buffers_for_plan(db, &plan)
            .into_iter()
            .map(|buffer| SyntaxTreeBuffer { path: buffer.path, text: buffer.text })
            .collect::<Vec<_>>(),
    );

    for root in &plan.roots {
        let path = compilation_plan::source_buffer_path(db, root.file_id).to_string();
        let name =
            db.file_path(root.file_id).map(|path| path.to_string()).unwrap_or_else(|| path.clone());
        let options = match root.kind {
            CompilationRootKind::SystemVerilog => SyntaxTreeOptions {
                predefines: context.predefines.to_vec(),
                include_paths: include_paths.clone(),
                ..SyntaxTreeOptions::default()
            },
            CompilationRootKind::LibraryMap => SyntaxTreeOptions::default(),
        };
        match root.kind {
            CompilationRootKind::SystemVerilog => {
                compilation.parse_syntax_tree_from_buffer(&name, &path, &options);
            }
            CompilationRootKind::LibraryMap => {
                compilation.parse_library_map_syntax_tree_from_buffer(&name, &path, &options);
            }
        }
    }
    compilation
}

#[cfg(test)]
mod tests {
    use base_db::{
        change::Change,
        project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
        source_root::{SourceRoot, SourceRootId},
    };
    use triomphe::Arc;
    use utils::{line_index::TextSize, paths::AbsPathBuf};
    use vfs::{ChangedFile, FileId, FileSet, VfsPath};

    use super::*;
    use crate::{
        analysis_host::AnalysisHost,
        test_utils::{setup_marked, setup_with_path},
    };

    const OBJECT: &str = r#"
virtual class uvm_void;
endclass
virtual class uvm_object extends uvm_void;
  string /*marker:name*/m_leaf_name;
endclass
"#;

    fn expect_ready<T: std::fmt::Debug>(result: ElabResult<T>) -> Option<T> {
        match result {
            ElabResult::Ready(value) => value,
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    /// Block for the build, then ask, so a cold snapshot cannot make an
    /// assertion about the *answer* fail for a latency reason.
    fn lookup_at(
        host: &AnalysisHost,
        file_id: FileId,
        offset: TextSize,
    ) -> ElabResult<SymbolInfo> {
        let ctx = host.ctx();
        let path = compilation_plan::source_buffer_path(ctx.db, file_id).to_string();
        let profile = ctx.db.file_compilation_profile(file_id);
        let built = ctx.elab.prewarm(ctx.db, ctx.revision);
        assert!(matches!(built, ElabResult::Ready(_)), "build must finish, got {built:?}");
        ctx.elab.lookup_symbol(ctx.db, ctx.revision, profile, &path, usize::from(offset))
    }

    #[test]
    fn ready_some_and_ready_none_are_distinct_from_stale_and_unavailable() {
        let (host, file_id, _text, markers) = setup_marked(OBJECT);
        let hit = lookup_at(&host, file_id, markers["name"]);
        let info = expect_ready(hit).expect("class property must be Ready(Some)");
        assert_eq!(info.owner_class, "uvm_object");
        assert!(info.inheritance.iter().any(|name| name == "uvm_void"), "{info:?}");
        assert!(info.type_name.contains("string"), "{info:?}");

        let miss = lookup_at(&host, file_id, TextSize::from(0u32));
        assert_eq!(miss, ElabResult::Ready(None), "a non-member offset is empty, not unavailable");
    }

    #[test]
    fn a_dropped_generation_is_stale_not_empty() {
        let (mut host, file_id) = setup_with_path(OBJECT, "/object.svh");
        let first = host.snapshot_id();
        let _ =
            lookup_at(&host, file_id, TextSize::from(OBJECT.find("m_leaf_name").unwrap() as u32));

        host.apply_change(modify_object("virtual class uvm_object extends uvm_void;\n  string m_leaf_name;\n  string extra;\nendclass\n"));
        let _ = lookup_at(&host, file_id, TextSize::from(0u32));

        host.apply_change(modify_object("virtual class uvm_object extends uvm_void;\n  string m_leaf_name;\n  string extra;\n  string extra2;\nendclass\n"));
        let _ = lookup_at(&host, file_id, TextSize::from(0u32));

        let ctx = host.ctx();
        let path = compilation_plan::source_buffer_path(ctx.db, file_id).to_string();
        let stale = ctx.elab.lookup_symbol(
            ctx.db,
            first,
            ctx.db.file_compilation_profile(file_id),
            &path,
            0,
        );
        match stale {
            ElabResult::Stale { want, .. } => assert_eq!(want, first),
            other => panic!("revision {first:?} must be Stale after N=2 rolled, got {other:?}"),
        }
    }

    #[test]
    fn a_dead_worker_is_unavailable_not_empty() {
        let (service, worker) = ElaborationService::spawn();
        service.shutdown();
        let _ = worker.join();
        let db = RootDb::new(None);
        let result = service.lookup_symbol(&db, AnalysisSnapshotId::default(), None, "gone.sv", 0);
        assert_eq!(
            result,
            ElabResult::Unavailable(UnavailableReason::WorkerGone),
            "a gone worker is WorkerGone, not empty"
        );
    }

    #[test]
    fn a_real_file_set_resolves_cross_file_inheritance() {
        let root = AbsPathBuf::assert(
            if cfg!(windows) { "C:/vide-elab-cross" } else { "/vide-elab-cross" }.into(),
        );
        let pkg_path = root.join("uvm_pkg.sv");
        let user_path = root.join("user.sv");
        let mut file_set = FileSet::default();
        file_set.insert(FileId::from_raw(0), VfsPath::from(pkg_path));
        file_set.insert(FileId::from_raw(1), VfsPath::from(user_path));

        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
            }],
        )));
        change.add_changed_file(ChangedFile::create(
            FileId::from_raw(0),
            "package uvm_pkg;\n  virtual class uvm_void;\n  endclass\n  virtual class uvm_object extends uvm_void;\n  endclass\nendpackage\n",
        ));
        let user = "package p;\n  import uvm_pkg::*;\n  class child extends uvm_object;\n    string m_leaf_name;\n  endclass\nendpackage\n";
        change.add_changed_file(ChangedFile::create(FileId::from_raw(1), user));

        let mut host = AnalysisHost::default();
        host.apply_change(change);
        let offset = TextSize::from(user.find("m_leaf_name").unwrap() as u32);
        let info = expect_ready(lookup_at(&host, FileId::from_raw(1), offset))
            .expect("cross-file class member");
        assert_eq!(info.owner_class, "child");
        assert!(
            info.inheritance.iter().any(|name| name == "uvm_object" || name == "uvm_void"),
            "inheritance must resolve through the imported package in the same compilation: {info:?}"
        );
    }

    #[test]
    fn instance_hierarchy_names_the_instantiation_site() {
        let src = "module child; endmodule\nmodule top; child u0(); endmodule\n";
        let (host, file_id) = setup_with_path(src, "/top.sv");
        let ctx = host.ctx();
        let rows = match ctx.elab.list_instances(
            ctx.db,
            ctx.revision,
            ctx.db.file_compilation_profile(file_id),
        ) {
            ElabResult::Ready(Some(rows)) => rows,
            other => panic!("expected instance list, got {other:?}"),
        };
        let u0 =
            rows.iter().find(|row| row.path.contains("u0")).unwrap_or_else(|| panic!("{rows:?}"));
        let site = src.find("u0").expect("instance name");
        assert_eq!(u0.offset, site, "{u0:?}");
        assert!(
            rows.iter().any(|row| row.offset == site && row.path.contains("u0")),
            "source site must list the instance: {rows:?}"
        );
    }

    #[test]
    /// Editing one root must leave the other root's symbols answerable.
    ///
    /// This used to assert that the untouched root kept its `SyntaxTree`.
    /// That reuse aborted the process — a tree belongs to the
    /// `SourceSession` of the `Compilation` that parsed it, and
    /// `add_syntax_tree` refuses a foreign one. What actually has to hold is
    /// the observable part: after an edit the new generation still answers
    /// for every root.
    fn an_edit_keeps_the_other_roots_answerable() {
        let root = AbsPathBuf::assert(
            if cfg!(windows) { "C:/vide-elab-reuse" } else { "/vide-elab-reuse" }.into(),
        );
        let class_file = FileId::from_raw(0);
        let module_file = FileId::from_raw(1);
        let mut file_set = FileSet::default();
        file_set.insert(class_file, VfsPath::from(root.join("a.sv")));
        file_set.insert(module_file, VfsPath::from(root.join("b.sv")));

        let class_text = "class holder;\n  string tag;\nendclass\n";
        let mut change = Change::new();
        change.set_roots(vec![SourceRoot::new_local(file_set)]);
        change.set_project_config(Arc::new(ProjectConfig::new(
            vec![Some(CompilationProfileId(0))],
            vec![CompilationProfile {
                source_roots: vec![SourceRootId(0)],
                top_modules: Vec::new(),
                preprocess: PreprocessConfig {
                    include_dirs: vec![root],
                    ..PreprocessConfig::default()
                },
            }],
        )));
        change.add_changed_file(ChangedFile::create(class_file, class_text));
        change.add_changed_file(ChangedFile::create(module_file, "module b;\nendmodule\n"));
        let mut host = AnalysisHost::default();
        host.apply_change(change);

        let tag = TextSize::from(class_text.find("tag").unwrap() as u32);
        let before = expect_ready(lookup_at(&host, class_file, tag)).expect("tag before the edit");
        assert_eq!(before.owner_class, "holder");

        let mut edit = Change::new();
        edit.add_changed_file(ChangedFile::modify(module_file, "module b;\n  wire w;\nendmodule\n"));
        host.apply_change(edit);

        let after = expect_ready(lookup_at(&host, class_file, tag)).expect("tag after the edit");
        assert_eq!(after, before, "an unrelated edit must not change this answer");
    }

    fn modify_object(text: &str) -> Change {
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(FileId::from_raw(0), text));
        change
    }
}
