//! Resident slang elaboration service (T4c).
//!
//! This is a backend worker, not a cache. Slang's `Compilation` is not
//! incremental, so the value does not belong in salsa or in
//! [`crate::incrementality::ProductStore`]. One worker thread owns the live
//! compilations; queries name a snapshot revision and get a typed result.
//!
//! `ElabResult` is the T7 safety rope: callers can tell "slang said nothing"
//! (`Ready(None)`) from "this snapshot is gone" (`Stale`) from "the worker
//! could not answer" (`Unavailable`). Silent `None` is a bug.

use std::{
    fmt,
    hash::{Hash, Hasher},
    panic::{self, AssertUnwindSafe},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use base_db::{
    analysis_snapshot::AnalysisSnapshotId, project::CompilationProfileId, source_db::SourceRootDb,
};
use preproc_expand::compilation_plan::{
    self, CompilationPlan, CompilationRootKind, compilation_source_buffers_for_plan,
};
use rustc_hash::{FxHashMap, FxHasher};
use slang_sys::compilation::{ClassMemberInfo, Compilation, HierInstance};
use syntax::{SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions};
use vfs::FileId;

use crate::db::root_db::RootDb;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(60);
const KEPT_GENERATIONS: usize = 2;

/// Snapshot tag carried by every query. Matches [`AnalysisSnapshotId`].
pub type ElabRevision = AnalysisSnapshotId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnavailableReason {
    NotReady,
    TimedOut,
    Crashed(String),
}

/// Answer from the resident compilation. The three arms are the contract:
/// empty, stale, and unavailable are not the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElabResult<T> {
    Ready(Option<T>),
    Stale { have: ElabRevision, want: ElabRevision },
    Unavailable(UnavailableReason),
}

#[derive(Clone)]
pub struct ElaborationService {
    tx: Sender<Request>,
}

impl fmt::Debug for ElaborationService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ElaborationService").finish()
    }
}

enum Request {
    Lookup {
        db: RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        path: String,
        offset: usize,
        reply: Sender<ElabResult<ClassMemberInfo>>,
    },
    Instances {
        db: RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        reply: Sender<ElabResult<Vec<HierInstance>>>,
    },
    #[cfg(test)]
    LastReused {
        reply: Sender<usize>,
    },
    Shutdown,
}

struct Generation {
    revision: ElabRevision,
    profiles: FxHashMap<Option<CompilationProfileId>, ProfileElab>,
    crash: Option<String>,
}

struct ProfileElab {
    compilation: Compilation,
    trees: FxHashMap<FileId, SyntaxTree>,
    file_hashes: FxHashMap<FileId, u64>,
    fingerprint: Fingerprint,
}

#[derive(Clone, PartialEq, Eq)]
struct Fingerprint {
    top_modules: Vec<String>,
    include_dirs: Vec<String>,
    predefines: Vec<String>,
    roots: Vec<(u32, CompilationRootKind)>,
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

    pub fn lookup_class_member(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
        path: &str,
        offset: usize,
    ) -> ElabResult<ClassMemberInfo> {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .tx
            .send(Request::Lookup {
                db: db.clone(),
                revision,
                profile,
                path: path.to_owned(),
                offset,
                reply: reply_tx,
            })
            .is_err()
        {
            return ElabResult::Unavailable(UnavailableReason::Crashed(
                "elaboration worker is gone".to_owned(),
            ));
        }
        match reply_rx.recv_timeout(LOOKUP_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                ElabResult::Unavailable(UnavailableReason::TimedOut)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => ElabResult::Unavailable(
                UnavailableReason::Crashed("elaboration worker dropped the lookup".to_owned()),
            ),
        }
    }

    pub fn list_instances(
        &self,
        db: &RootDb,
        revision: ElabRevision,
        profile: Option<CompilationProfileId>,
    ) -> ElabResult<Vec<HierInstance>> {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self
            .tx
            .send(Request::Instances { db: db.clone(), revision, profile, reply: reply_tx })
            .is_err()
        {
            return ElabResult::Unavailable(UnavailableReason::Crashed(
                "elaboration worker is gone".to_owned(),
            ));
        }
        match reply_rx.recv_timeout(LOOKUP_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                ElabResult::Unavailable(UnavailableReason::TimedOut)
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => ElabResult::Unavailable(
                UnavailableReason::Crashed("elaboration worker dropped the lookup".to_owned()),
            ),
        }
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(Request::Shutdown);
    }

    #[cfg(test)]
    pub(crate) fn last_reused_root_count(&self) -> usize {
        let (reply_tx, reply_rx) = mpsc::channel();
        if self.tx.send(Request::LastReused { reply: reply_tx }).is_err() {
            return 0;
        }
        reply_rx.recv_timeout(LOOKUP_TIMEOUT).unwrap_or(0)
    }
}

fn worker_loop(rx: Receiver<Request>) {
    let mut gens: Vec<Generation> = Vec::new();
    let mut last_reused = 0usize;
    while let Ok(request) = rx.recv() {
        match request {
            Request::Lookup { db, revision, profile, path, offset, reply } => {
                let result =
                    handle_lookup(&mut gens, &mut last_reused, db, revision, profile, path, offset);
                let _ = reply.send(result);
            }
            Request::Instances { db, revision, profile, reply } => {
                let result = handle_instances(&mut gens, &mut last_reused, db, revision, profile);
                let _ = reply.send(result);
            }
            #[cfg(test)]
            Request::LastReused { reply } => {
                let _ = reply.send(last_reused);
            }
            Request::Shutdown => break,
        }
    }
}

fn handle_lookup(
    gens: &mut Vec<Generation>,
    last_reused: &mut usize,
    db: RootDb,
    revision: ElabRevision,
    profile: Option<CompilationProfileId>,
    path: String,
    offset: usize,
) -> ElabResult<ClassMemberInfo> {
    if !gens.iter().any(|slot| slot.revision == revision) {
        if !should_build(gens, revision) {
            let have = gens.last().map(|slot| slot.revision).unwrap_or(revision);
            return ElabResult::Stale { have, want: revision };
        }
        match panic::catch_unwind(AssertUnwindSafe(|| rebuild(&db, revision, gens))) {
            Ok((built, reused)) => {
                *last_reused = reused;
                gens.push(built);
                if gens.len() > KEPT_GENERATIONS {
                    gens.remove(0);
                }
            }
            Err(_) => {
                gens.push(Generation {
                    revision,
                    profiles: FxHashMap::default(),
                    crash: Some("elaboration rebuild panicked".to_owned()),
                });
                if gens.len() > KEPT_GENERATIONS {
                    gens.remove(0);
                }
            }
        }
    }

    let Some(slot) = gens.iter_mut().find(|slot| slot.revision == revision) else {
        return ElabResult::Unavailable(UnavailableReason::NotReady);
    };
    if let Some(message) = &slot.crash {
        return ElabResult::Unavailable(UnavailableReason::Crashed(message.clone()));
    }
    let Some(profile_elab) = slot.profiles.get_mut(&profile) else {
        return ElabResult::Unavailable(UnavailableReason::NotReady);
    };
    match panic::catch_unwind(AssertUnwindSafe(|| {
        profile_elab.compilation.lookup_class_member(&path, offset)
    })) {
        Ok(answer) => ElabResult::Ready(answer),
        Err(_) => ElabResult::Unavailable(UnavailableReason::Crashed(
            "class-member lookup panicked".to_owned(),
        )),
    }
}

fn handle_instances(
    gens: &mut Vec<Generation>,
    last_reused: &mut usize,
    db: RootDb,
    revision: ElabRevision,
    profile: Option<CompilationProfileId>,
) -> ElabResult<Vec<HierInstance>> {
    match handle_lookup(gens, last_reused, db, revision, profile, String::new(), 0) {
        ElabResult::Stale { have, want } => ElabResult::Stale { have, want },
        ElabResult::Unavailable(reason) => ElabResult::Unavailable(reason),
        ElabResult::Ready(_) => {
            let slot = gens.iter_mut().find(|slot| slot.revision == revision);
            let Some(slot) = slot else {
                return ElabResult::Unavailable(UnavailableReason::NotReady);
            };
            let Some(profile_elab) = slot.profiles.get_mut(&profile) else {
                return ElabResult::Unavailable(UnavailableReason::NotReady);
            };
            match panic::catch_unwind(AssertUnwindSafe(|| {
                profile_elab.compilation.list_instances()
            })) {
                Ok(instances) => ElabResult::Ready(Some(instances)),
                Err(_) => ElabResult::Unavailable(UnavailableReason::Crashed(
                    "instance walk panicked".to_owned(),
                )),
            }
        }
    }
}

fn should_build(gens: &[Generation], revision: ElabRevision) -> bool {
    gens.is_empty() || gens.iter().all(|slot| slot.revision < revision)
}

fn rebuild(db: &RootDb, revision: ElabRevision, prev: &[Generation]) -> (Generation, usize) {
    let profile_ids = {
        let ids = db.project_config().profile_ids();
        if ids.is_empty() { vec![None] } else { ids.into_iter().map(Some).collect() }
    };
    let reuse_from = prev.last();
    let mut profiles = FxHashMap::default();
    let mut reused_total = 0;
    for profile_id in profile_ids {
        let previous = reuse_from.and_then(|slot| slot.profiles.get(&profile_id));
        let (elab, reused) = compile_profile(db, profile_id, previous);
        reused_total += reused;
        profiles.insert(profile_id, elab);
    }
    (Generation { revision, profiles, crash: None }, reused_total)
}

fn compile_profile(
    db: &RootDb,
    profile_id: Option<CompilationProfileId>,
    prev: Option<&ProfileElab>,
) -> (ProfileElab, usize) {
    let plan = db.compilation_plan_for_profile(profile_id);
    let context = db.compilation_context(profile_id);
    let buffers = compilation_source_buffers_for_plan(db, &plan);
    let fingerprint = Fingerprint {
        top_modules: context.top_modules.to_vec(),
        include_dirs: context.include_dirs.iter().map(ToString::to_string).collect(),
        predefines: context.predefines.to_vec(),
        roots: plan.roots.iter().map(|root| (root.file_id.index(), root.kind)).collect(),
    };
    let new_hashes: FxHashMap<FileId, u64> =
        buffers.iter().map(|buffer| (buffer.file_id, hash_text(&buffer.text))).collect();
    let can_reuse = prev.is_some_and(|previous| previous.fingerprint == fingerprint);

    let mut compilation = Compilation::new_with_top_modules(&fingerprint.top_modules);
    compilation.register_source_buffers(
        &buffers
            .iter()
            .map(|buffer| SyntaxTreeBuffer { path: buffer.path.clone(), text: buffer.text.clone() })
            .collect::<Vec<_>>(),
    );

    let mut trees = FxHashMap::default();
    let mut reused = 0;
    for root in &plan.roots {
        let previous = prev.filter(|_| can_reuse);
        let dirty = previous.is_none_or(|previous| {
            root_is_dirty(root.file_id, &plan, &previous.file_hashes, &new_hashes)
        });
        if !dirty {
            if let Some(tree) = previous.and_then(|previous| previous.trees.get(&root.file_id)) {
                compilation.add_syntax_tree(tree);
                trees.insert(root.file_id, tree.clone());
                reused += 1;
                continue;
            }
        }
        let path = compilation_plan::source_buffer_path(db, root.file_id).to_string();
        let name =
            db.file_path(root.file_id).map(|path| path.to_string()).unwrap_or_else(|| path.clone());
        let tree = match root.kind {
            CompilationRootKind::SystemVerilog => {
                let options = SyntaxTreeOptions {
                    predefines: fingerprint.predefines.clone(),
                    include_paths: fingerprint.include_dirs.clone(),
                    ..SyntaxTreeOptions::default()
                };
                compilation.parse_syntax_tree_from_buffer(&name, &path, &options)
            }
            CompilationRootKind::LibraryMap => compilation
                .parse_library_map_syntax_tree_from_buffer(
                    &name,
                    &path,
                    &SyntaxTreeOptions::default(),
                ),
        };
        trees.insert(root.file_id, tree);
    }

    (ProfileElab { compilation, trees, file_hashes: new_hashes, fingerprint }, reused)
}

fn root_is_dirty(
    root: FileId,
    plan: &CompilationPlan,
    old_hashes: &FxHashMap<FileId, u64>,
    new_hashes: &FxHashMap<FileId, u64>,
) -> bool {
    if old_hashes.get(&root) != new_hashes.get(&root) {
        return true;
    }
    match plan.include_closure(root) {
        Some(closure) => closure.iter().any(|file| old_hashes.get(file) != new_hashes.get(file)),
        None => old_hashes != new_hashes,
    }
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = FxHasher::default();
    text.hash(&mut hasher);
    hasher.finish()
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

    fn lookup_at(
        host: &AnalysisHost,
        file_id: FileId,
        offset: TextSize,
    ) -> ElabResult<ClassMemberInfo> {
        let ctx = host.ctx();
        let path = compilation_plan::source_buffer_path(ctx.db, file_id).to_string();
        let profile = ctx.db.file_compilation_profile(file_id);
        ctx.elab.lookup_class_member(ctx.db, ctx.revision, profile, &path, usize::from(offset))
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
        let stale = ctx.elab.lookup_class_member(
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
        let result =
            service.lookup_class_member(&db, AnalysisSnapshotId::default(), None, "gone.sv", 0);
        assert!(
            matches!(result, ElabResult::Unavailable(UnavailableReason::Crashed(_))),
            "a gone worker is Unavailable, got {result:?}"
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
    fn an_edit_reuses_the_unchanged_root_tree() {
        let root = AbsPathBuf::assert(
            if cfg!(windows) { "C:/vide-elab-reuse" } else { "/vide-elab-reuse" }.into(),
        );
        let a_path = root.join("a.sv");
        let b_path = root.join("b.sv");
        let mut file_set = FileSet::default();
        file_set.insert(FileId::from_raw(0), VfsPath::from(a_path));
        file_set.insert(FileId::from_raw(1), VfsPath::from(b_path));

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
            "virtual class uvm_void;\nendclass\n",
        ));
        change.add_changed_file(ChangedFile::create(FileId::from_raw(1), "module b;\nendmodule\n"));
        let mut host = AnalysisHost::default();
        host.apply_change(change);
        let _ = lookup_at(&host, FileId::from_raw(1), TextSize::from(0u32));
        assert_eq!(host.elab().last_reused_root_count(), 0);

        let mut edit = Change::new();
        edit.add_changed_file(ChangedFile::modify(
            FileId::from_raw(1),
            "module b;\n  wire w;\nendmodule\n",
        ));
        host.apply_change(edit);
        let _ = lookup_at(&host, FileId::from_raw(1), TextSize::from(0u32));
        assert_eq!(
            host.elab().last_reused_root_count(),
            1,
            "the unchanged class file must keep its SyntaxTree"
        );
    }

    fn modify_object(text: &str) -> Change {
        let mut change = Change::new();
        change.add_changed_file(ChangedFile::modify(FileId::from_raw(0), text));
        change
    }
}
