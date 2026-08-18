use base_db::{
    project::{CompilationProfileId, ProjectConfig},
    source_db::{SourceFileKind, SourceRootDb},
    source_root::SourceRootId,
};
use preproc::source::{
    MacroIncludeTarget, SourceIncludeDirective, SourcePreprocError, SourcePreprocModel,
};
use rustc_hash::{FxHashMap, FxHashSet};
use syntax::{SyntaxTree, SyntaxTreeBuffer, SyntaxTreeOptions};
use utils::{
    path_identity::PathIdentityIndex,
    paths::{AbsPath, AbsPathBuf, Utf8Path, Utf8PathBuf},
};
use vfs::FileId;

use crate::db::PreprocDb;

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
struct IncludeScanQueryKey {
    file_id: FileId,
    predefines: triomphe::Arc<[String]>,
}

/// A resolved literal `` `include ``. `slang_path` is the cache key slang will
/// use for this edge (`parent(from) / literal` when that join is the target
/// file, otherwise the target's VFS path for an include-dir hit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeEdge {
    pub from: FileId,
    pub to: FileId,
    pub literal: String,
    pub slang_path: AbsPathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompilationPlan {
    pub source_roots: Vec<SourceRootId>,
    pub roots: Vec<FileId>,
    /// Files reached through literal SystemVerilog include directives. They are
    /// made available to slang through include buffers, but are not added
    /// as standalone semantic roots.
    pub include_only: FxHashSet<FileId>,
    /// Direct resolved include edges, keyed by the including file.
    pub include_dependencies: FxHashMap<FileId, FxHashSet<FileId>>,
    /// Resolved include edges with the slang lookup spelling for each use.
    pub include_edges: Vec<IncludeEdge>,
    /// Files with a non-literal include target. Their exact dependency cannot
    /// be known without the authoritative preprocessor, so they are treated as
    /// affected by every source edit.
    pub dynamic_include_files: FxHashSet<FileId>,
    pub include_dirs: Vec<AbsPathBuf>,
    pub top_modules: Vec<String>,
    pub predefines: Vec<String>,
    pub include_scan_issues: Vec<IncludeScanIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeScanIssue {
    pub file_id: FileId,
    pub reason: IncludeScanIssueReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeScanIssueReason {
    Model(SourcePreprocError),
}

impl CompilationPlan {
    /// Every file the plan compiles: semantic roots plus include-only files,
    /// in stable order without duplicates.
    pub fn all_file_ids(&self) -> Vec<FileId> {
        let mut file_ids = self.roots.clone();
        file_ids.extend(self.include_only.iter().copied());
        file_ids.sort_unstable_by_key(|file_id| file_id.index());
        file_ids.dedup();
        file_ids
    }

    /// Return changed files plus every file that transitively includes one of
    /// them in this compilation plan.
    pub fn affected_files(&self, changed: impl IntoIterator<Item = FileId>) -> FxHashSet<FileId> {
        let mut affected = changed.into_iter().collect::<FxHashSet<_>>();
        loop {
            let mut grew = false;
            for (&includer, dependencies) in &self.include_dependencies {
                if !affected.contains(&includer)
                    && dependencies.iter().any(|dependency| affected.contains(dependency))
                {
                    affected.insert(includer);
                    grew = true;
                }
            }
            if !grew {
                return affected;
            }
        }
    }

    /// Exact transitive include closure when every visited directive resolved
    /// statically. Dynamic or currently missing include targets return `None`,
    /// which tells the parser to retain the conservative profile-wide buffer
    /// set for correctness.
    pub fn include_closure(&self, root: FileId) -> Option<FxHashSet<FileId>> {
        let mut closure = FxHashSet::default();
        let mut pending = vec![root];
        while let Some(file_id) = pending.pop() {
            if self.dynamic_include_files.contains(&file_id) {
                return None;
            }
            let Some(dependencies) = self.include_dependencies.get(&file_id) else {
                continue;
            };
            for &dependency in dependencies {
                if closure.insert(dependency) {
                    pending.push(dependency);
                }
            }
        }
        Some(closure)
    }

    /// Whether a file should be made available to slang as an include buffer:
    /// include headers reachable through the configured include paths.
    pub fn is_include_header_in_include_paths(
        &self,
        db: &dyn SourceRootDb,
        file_id: FileId,
    ) -> bool {
        matches!(db.file_kind(file_id), SourceFileKind::IncludeHeader)
            && db.file_path(file_id).is_some_and(|path| {
                self.include_dirs.iter().any(|include_dir| path.starts_with(include_dir))
            })
    }

    pub fn for_source_root(db: &dyn PreprocDb, source_root_id: SourceRootId) -> Self {
        let project_config = db.project_config();
        let profile_id = project_config.profile_for_root(source_root_id);
        // Profile-backed plans are the normal project path. A compile-capable
        // Local/Library root can still produce a root-scoped plan when it has
        // no attached profile, for example in a workspace without a manifest.
        let root_scoped_source_root = db
            .source_root(source_root_id)
            .role()
            .supports_root_scoped_compilation()
            .then_some(source_root_id);
        let (source_roots, top_modules, include_dirs, predefines) =
            profile_inputs(&project_config, root_scoped_source_root, profile_id);
        Self::from_inputs(db, source_roots, top_modules, include_dirs, predefines)
    }

    pub fn for_profile(db: &dyn PreprocDb, profile_id: Option<CompilationProfileId>) -> Self {
        let project_config = db.project_config();
        let (source_roots, top_modules, include_dirs, predefines) =
            profile_inputs(&project_config, None, profile_id);
        let source_roots =
            if source_roots.is_empty() { all_non_ignored_roots(db) } else { source_roots };
        Self::from_inputs(db, source_roots, top_modules, include_dirs, predefines)
    }

    fn from_inputs(
        db: &dyn PreprocDb,
        source_roots: Vec<SourceRootId>,
        top_modules: Vec<String>,
        include_dirs: Vec<AbsPathBuf>,
        predefines: Vec<String>,
    ) -> Self {
        let mut starts = Vec::new();
        for root in &source_roots {
            starts.extend(db.source_root(*root).iter());
        }
        let scan = scan_include_graph(db, starts, &include_dirs, &predefines);
        let (include_only, include_dependencies) = include_projections(&scan.edges);
        let roots = compile_roots_for_source_roots(db, &source_roots, &include_only);
        CompilationPlan {
            source_roots,
            roots,
            include_only,
            include_dependencies,
            include_edges: scan.edges,
            dynamic_include_files: scan.dynamic_files,
            include_dirs,
            top_modules,
            predefines,
            include_scan_issues: scan.issues,
        }
    }
}

pub fn include_buffers_for_plan(
    db: &dyn PreprocDb,
    plan: &CompilationPlan,
) -> Vec<SyntaxTreeBuffer> {
    include_buffers_for_plan_with_roots(db, plan, false)
        .into_iter()
        .map(|buffer| SyntaxTreeBuffer { path: buffer.path, text: buffer.text })
        .collect()
}

/// A source buffer we hand to slang, keyed by the spelling slang will look up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedIncludeBuffer {
    pub file_id: FileId,
    pub path: String,
    pub text: String,
}

/// Transitive literal includes of one file, walking only that file's
/// include graph. Dynamic or unresolved directives make the closure
/// [`Partial`](StaticIncludeClosure::Partial); resolved files are still
/// returned. This never expands to the whole profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticIncludeClosure {
    Complete(Vec<FileId>),
    Partial(Vec<FileId>),
}

impl StaticIncludeClosure {
    pub fn files(&self) -> &[FileId] {
        match self {
            Self::Complete(files) | Self::Partial(files) => files,
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}

/// Include buffers needed by one standalone compilation unit.
///
/// Each resolved include is registered under that edge's `slang_path` only. A
/// dynamic or unresolved include does **not** load every header in the
/// profile.
pub fn include_buffers_for_file(db: &dyn PreprocDb, file_id: FileId) -> Vec<SyntaxTreeBuffer> {
    assigned_include_buffers_for_file(db, file_id)
        .into_iter()
        .map(|buffer| SyntaxTreeBuffer { path: buffer.path, text: buffer.text })
        .collect()
}

pub fn assigned_include_buffers_for_file(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> Vec<AssignedIncludeBuffer> {
    buffers_from_edges(db, &scan_includes_from_file(db, file_id).edges)
}

/// Walk literal `` `include `` directives from `file_id` only.
pub fn static_include_closure(db: &dyn PreprocDb, file_id: FileId) -> StaticIncludeClosure {
    let scan = scan_includes_from_file(db, file_id);
    let (include_only, _) = include_projections(&scan.edges);
    let mut files = include_only.into_iter().collect::<Vec<_>>();
    files.sort_unstable_by_key(|file_id| file_id.index());
    if scan.complete {
        StaticIncludeClosure::Complete(files)
    } else {
        StaticIncludeClosure::Partial(files)
    }
}

pub fn compilation_source_buffers_for_plan(
    db: &dyn PreprocDb,
    plan: &CompilationPlan,
) -> Vec<AssignedIncludeBuffer> {
    include_buffers_for_plan_with_roots(db, plan, true)
}

/// Return the stable path Slang should use for a source file buffer.
///
/// Test fixtures and editor-only files can have no filesystem path. They still
/// need a non-empty identity because `SourceManager::assignText` and
/// `SyntaxTree::fromBuffer` are path keyed. Keep those identities deterministic
/// within the database snapshot instead of falling back to an empty path.
pub fn source_buffer_path(db: &dyn SourceRootDb, file_id: FileId) -> AbsPathBuf {
    db.file_path(file_id).unwrap_or_else(|| synthetic_source_buffer_path(file_id))
}

fn synthetic_source_buffer_path(file_id: FileId) -> AbsPathBuf {
    let root = if cfg!(windows) {
        Utf8PathBuf::from(r"C:\__vide_virtual__")
    } else {
        Utf8PathBuf::from("/__vide_virtual__")
    };
    AbsPathBuf::try_from(root.join(file_id.index().to_string()))
        .expect("synthetic source buffer path must be absolute and UTF-8")
}

fn include_buffers_for_plan_with_roots(
    db: &dyn PreprocDb,
    plan: &CompilationPlan,
    include_roots: bool,
) -> Vec<AssignedIncludeBuffer> {
    let root_files = if include_roots {
        plan.roots.iter().copied().collect::<FxHashSet<_>>()
    } else {
        FxHashSet::default()
    };
    let mut seen_files = PathIdentityIndex::default();
    let mut seen_buffer_paths = FxHashSet::default();
    let mut buffers = Vec::new();

    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }

        let include_header_in_include_path = plan.is_include_header_in_include_paths(db, file_id);
        let semantic_root = root_files.contains(&file_id)
            && matches!(
                db.file_kind(file_id),
                SourceFileKind::SystemVerilog | SourceFileKind::LibraryMap
            );
        if !semantic_root
            && !include_header_in_include_path
            && !plan.include_only.contains(&file_id)
        {
            continue;
        }

        let path = source_buffer_path(db, file_id);
        if let Some(existing_file_id) = seen_files.get_path(&path) {
            if db.file_text(existing_file_id) != db.file_text(file_id) {
                panic!(
                    "source buffer path has conflicting texts: {} ({existing_file_id:?}, {file_id:?})",
                    path
                );
            }
            continue;
        }
        seen_files.insert_path(&path, file_id);

        let path = path.to_string();
        if seen_buffer_paths.insert(path.clone()) {
            buffers.push(AssignedIncludeBuffer {
                file_id,
                path,
                text: db.file_text(file_id).to_string(),
            });
        }
    }

    for buffer in buffers_from_edges(db, &plan.include_edges) {
        if seen_buffer_paths.insert(buffer.path.clone()) {
            buffers.push(buffer);
        }
    }

    buffers
}

/// FFI path → [`FileId`] for one standalone parse: the root's VFS spelling plus
/// each reachable include edge's `slang_path`.
pub(crate) fn source_buffer_file_ids_for_file(
    db: &dyn PreprocDb,
    file_id: FileId,
) -> PathIdentityIndex<FileId> {
    let mut index = PathIdentityIndex::default();
    index.insert_path(source_buffer_path(db, file_id).as_path(), file_id);
    for edge in scan_includes_from_file(db, file_id).edges {
        index.insert_path(edge.slang_path.as_path(), edge.to);
    }
    index
}

fn scan_includes_from_file(db: &dyn PreprocDb, file_id: FileId) -> IncludeScan {
    let preprocess =
        db.project_config().preprocess_for_profile(db.file_compilation_profile(file_id));
    scan_include_graph(db, [file_id], &preprocess.include_dirs, &preprocess.predefine_strings())
}

fn buffers_from_edges(db: &dyn PreprocDb, edges: &[IncludeEdge]) -> Vec<AssignedIncludeBuffer> {
    let mut seen_paths = FxHashSet::default();
    let mut buffers = Vec::new();
    for edge in edges {
        if db.file_is_project_ignored(edge.to) {
            continue;
        }
        let path = edge.slang_path.to_string();
        if !seen_paths.insert(path.clone()) {
            continue;
        }
        buffers.push(AssignedIncludeBuffer {
            file_id: edge.to,
            path,
            text: db.file_text(edge.to).to_string(),
        });
    }
    buffers
}

fn include_projections(
    edges: &[IncludeEdge],
) -> (FxHashSet<FileId>, FxHashMap<FileId, FxHashSet<FileId>>) {
    let mut include_only = FxHashSet::default();
    let mut include_dependencies = FxHashMap::<FileId, FxHashSet<FileId>>::default();
    for edge in edges {
        include_only.insert(edge.to);
        include_dependencies.entry(edge.from).or_default().insert(edge.to);
    }
    (include_only, include_dependencies)
}

/// Slang's first include lookup key when `disableProximatePaths` is set:
/// `parent(includer) / include-literal`, with no `.`/`..` collapse.
pub(crate) fn slang_local_include_lookup_path(
    includer: &AbsPath,
    literal: &str,
) -> Option<AbsPathBuf> {
    let include = Utf8Path::new(literal);
    if include.is_absolute() {
        return AbsPathBuf::try_from(include.to_path_buf()).ok();
    }
    let dir = includer.parent()?;
    AbsPathBuf::try_from(Utf8Path::new(dir.as_str()).join(include)).ok()
}

/// The spelling to hand slang for one resolved include.
fn slang_path_for_include(includer: &AbsPath, literal: &str, target_vfs: &AbsPath) -> AbsPathBuf {
    let Some(local) = slang_local_include_lookup_path(includer, literal) else {
        return target_vfs.to_path_buf();
    };
    if local.normalize() == target_vfs.normalize() { local } else { target_vfs.to_path_buf() }
}

fn profile_inputs(
    project_config: &ProjectConfig,
    root_scoped_source_root: Option<SourceRootId>,
    profile_id: Option<CompilationProfileId>,
) -> (Vec<SourceRootId>, Vec<String>, Vec<AbsPathBuf>, Vec<String>) {
    if let Some(profile) = profile_id.and_then(|profile_id| project_config.profile(profile_id)) {
        return (
            profile.source_roots.clone(),
            profile.top_modules.clone(),
            profile.preprocess.include_dirs.clone(),
            profile.preprocess.predefine_strings(),
        );
    }

    let preprocess = project_config.preprocess_for_profile(profile_id);
    let predefines = preprocess.predefine_strings();
    (root_scoped_source_root.into_iter().collect(), Vec::new(), preprocess.include_dirs, predefines)
}

fn all_non_ignored_roots(db: &dyn SourceRootDb) -> Vec<SourceRootId> {
    let mut roots = FxHashSet::default();
    for file_id in db.files().iter().copied() {
        if !db.file_is_project_ignored(file_id) {
            let source_root_id = db.source_root_id(file_id);
            if db.source_root(source_root_id).role().supports_root_scoped_compilation() {
                roots.insert(source_root_id);
            }
        }
    }
    roots.into_iter().collect()
}

fn compile_roots_for_source_roots(
    db: &dyn SourceRootDb,
    roots: &[SourceRootId],
    include_only: &FxHashSet<FileId>,
) -> Vec<FileId> {
    let mut files = Vec::new();
    let mut visited = FxHashSet::default();

    for root_id in roots {
        let source_root = db.source_root(*root_id);
        for file_id in source_root.iter() {
            if !visited.insert(file_id) {
                continue;
            }
            if db.file_is_project_ignored(file_id) {
                continue;
            }
            if !db.file_kind(file_id).is_semantic_compilation_unit() {
                continue;
            }
            if matches!(db.file_kind(file_id), SourceFileKind::SystemVerilog)
                && include_only.contains(&file_id)
            {
                continue;
            }
            files.push(file_id);
        }
    }

    files
}

fn path_file_ids(db: &dyn SourceRootDb) -> PathIdentityIndex<FileId> {
    let mut index = PathIdentityIndex::default();
    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }
        if let Some(path) = db.file_path(file_id) {
            index.insert_path(&path, file_id);
        }
    }
    index
}

struct IncludeScan {
    edges: Vec<IncludeEdge>,
    dynamic_files: FxHashSet<FileId>,
    issues: Vec<IncludeScanIssue>,
    complete: bool,
}

fn scan_include_graph(
    db: &dyn PreprocDb,
    starts: impl IntoIterator<Item = FileId>,
    include_dirs: &[AbsPathBuf],
    predefines: &[String],
) -> IncludeScan {
    let path_file_ids = path_file_ids(db);
    let predefines = triomphe::Arc::<[String]>::from(predefines.to_vec());
    let mut edges = Vec::new();
    let mut seen_edges = FxHashSet::default();
    let mut dynamic_files = FxHashSet::default();
    let mut issues = Vec::new();
    let mut complete = true;
    let mut scanned = FxHashSet::default();
    let mut pending = starts.into_iter().collect::<Vec<_>>();

    while let Some(file_id) = pending.pop() {
        if !scanned.insert(file_id) {
            continue;
        }
        if db.file_is_project_ignored(file_id) {
            continue;
        }
        if !matches!(
            db.file_kind(file_id),
            SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
        ) {
            continue;
        }

        let includer_path =
            db.file_path(file_id).unwrap_or_else(|| source_buffer_path(db, file_id));

        let include_targets = match literal_include_targets(
            db,
            IncludeScanQueryKey::new(db, file_id, predefines.clone()),
        ) {
            Ok(targets) => targets,
            Err(issue) => {
                complete = false;
                dynamic_files.insert(file_id);
                issues.push(issue);
                continue;
            }
        };
        for include in include_targets {
            let MacroIncludeTarget::Literal { path, .. } = &include.target else {
                complete = false;
                dynamic_files.insert(file_id);
                continue;
            };
            let Some(to) =
                resolve_include_target(path.as_str(), &includer_path, include_dirs, &path_file_ids)
            else {
                complete = false;
                dynamic_files.insert(file_id);
                continue;
            };
            pending.push(to);
            if db.file_is_project_ignored(to) {
                continue;
            }
            let target_vfs = source_buffer_path(db, to);
            let slang_path = slang_path_for_include(
                includer_path.as_path(),
                path.as_str(),
                target_vfs.as_path(),
            );
            if seen_edges.insert((file_id, to, slang_path.clone())) {
                edges.push(IncludeEdge {
                    from: file_id,
                    to,
                    literal: path.to_string(),
                    slang_path,
                });
            }
        }
    }

    IncludeScan { edges, dynamic_files, issues, complete }
}

#[salsa::tracked(returns(clone))]
fn literal_include_targets(
    db: &dyn PreprocDb,
    key: IncludeScanQueryKey,
) -> Result<Vec<SourceIncludeDirective>, IncludeScanIssue> {
    let file_id = *key.file_id(db);
    let predefines = key.predefines(db);
    if !matches!(
        db.file_kind(file_id),
        SourceFileKind::SystemVerilog | SourceFileKind::IncludeHeader
    ) {
        return Ok(Vec::new());
    }

    let path = db.file_path(file_id).map(|path| path.to_string()).unwrap_or_default();
    let name = if path.is_empty() { "source".to_owned() } else { path.clone() };
    let options = SyntaxTreeOptions {
        predefines: predefines.to_vec(),
        ..SyntaxTreeOptions::without_include_expansion()
    };
    let parsed = SyntaxTree::from_file_in_memory_with_options_and_trace(
        &db.file_text(file_id),
        &name,
        &path,
        &options,
    );
    let trace = parsed.preprocessor_trace;
    let model = SourcePreprocModel::from_trace(&trace)
        .map_err(|err| IncludeScanIssue { file_id, reason: IncludeScanIssueReason::Model(err) })?;
    Ok(model.include_graph().directives().to_vec())
}

fn resolve_include_target(
    path: &str,
    includer_path: &AbsPathBuf,
    include_dirs: &[AbsPathBuf],
    path_file_ids: &PathIdentityIndex<FileId>,
) -> Option<FileId> {
    let include_path = Utf8Path::new(path);
    if include_path.is_absolute() {
        let abs_path = AbsPathBuf::try_from(include_path.to_path_buf()).ok()?.normalize();
        return path_file_ids.get_path(abs_path.as_path());
    }

    if let Some(parent) = includer_path.parent() {
        let candidate = parent.absolutize(include_path);
        if let Some(file_id) = path_file_ids.get_path(candidate.as_path()) {
            return Some(file_id);
        }
    }

    for include_dir in include_dirs {
        let candidate = include_dir.absolutize(include_path);
        if let Some(file_id) = path_file_ids.get_path(candidate.as_path()) {
            return Some(file_id);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slang_path_uses_local_join_when_it_names_the_target() {
        let includer = if cfg!(windows) {
            AbsPathBuf::assert(r"C:\repo\rtl\darkcache.v".into())
        } else {
            AbsPathBuf::assert("/repo/rtl/darkcache.v".into())
        };
        let target = if cfg!(windows) {
            AbsPathBuf::assert(r"C:\repo\rtl\config.vh".into())
        } else {
            AbsPathBuf::assert("/repo/rtl/config.vh".into())
        };
        let path = slang_path_for_include(includer.as_path(), "../rtl/config.vh", target.as_path());
        let path = path.to_string().replace('\\', "/");
        assert!(
            path.ends_with("rtl/../rtl/config.vh"),
            "same-file local join is the slang lookup key: {path}"
        );
    }

    #[test]
    fn slang_path_uses_vfs_path_when_include_dirs_resolve_the_target() {
        let includer = if cfg!(windows) {
            AbsPathBuf::assert(r"C:\repo\rtl\top.v".into())
        } else {
            AbsPathBuf::assert("/repo/rtl/top.v".into())
        };
        let target = if cfg!(windows) {
            AbsPathBuf::assert(r"C:\repo\include\defs.vh".into())
        } else {
            AbsPathBuf::assert("/repo/include/defs.vh".into())
        };
        let path = slang_path_for_include(includer.as_path(), "defs.vh", target.as_path());
        assert_eq!(path.as_path(), target.as_path());
    }

    #[test]
    fn slang_local_include_lookup_keeps_parent_segments() {
        let includer = if cfg!(windows) {
            AbsPathBuf::assert(r"C:\repo\rtl\darkcache.v".into())
        } else {
            AbsPathBuf::assert("/repo/rtl/darkcache.v".into())
        };
        let lookup = slang_local_include_lookup_path(includer.as_path(), "../rtl/config.vh")
            .expect("relative include must produce a lookup path");
        let lookup = lookup.to_string().replace('\\', "/");
        assert!(
            lookup.ends_with("rtl/../rtl/config.vh"),
            "slang lookup key must keep the include join: {lookup}"
        );
    }

    #[test]
    fn include_closure_contains_only_transitive_dependencies() {
        let root = FileId::from_raw(0);
        let direct = FileId::from_raw(1);
        let transitive = FileId::from_raw(2);
        let unrelated = FileId::from_raw(3);
        let mut plan = CompilationPlan::default();
        plan.include_dependencies.insert(root, FxHashSet::from_iter([direct]));
        plan.include_dependencies.insert(direct, FxHashSet::from_iter([transitive]));
        plan.include_dependencies.insert(unrelated, FxHashSet::default());

        let closure = plan.include_closure(root).unwrap();

        assert_eq!(closure, FxHashSet::from_iter([direct, transitive]));
        assert!(!closure.contains(&unrelated));
    }

    #[test]
    fn dynamic_include_forces_conservative_manifest() {
        let root = FileId::from_raw(0);
        let mut plan = CompilationPlan::default();
        plan.dynamic_include_files.insert(root);

        assert_eq!(plan.include_closure(root), None);
    }

    #[test]
    fn synthetic_source_buffer_paths_are_absolute() {
        let path = synthetic_source_buffer_path(FileId::from_raw(0));

        assert!(Utf8Path::new(path.as_str()).is_absolute());
        assert!(path.ends_with(utils::paths::RelPath::new_unchecked(Utf8Path::new("0"))));
    }
}
