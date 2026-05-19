use rustc_hash::FxHashSet;
use syntax::SyntaxTreeBuffer;
use utils::{
    path_identity::{PathIdentityIndex, PathIdentitySet},
    paths::{AbsPathBuf, Utf8Path},
};
use vfs::FileId;

use crate::{
    preproc_index::MacroIncludeTarget,
    project::{CompilationProfileId, ProjectConfig},
    source_db::{SourceFileKind, SourceRootDb},
    source_root::SourceRootId,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompilationPlan {
    pub source_roots: Vec<SourceRootId>,
    pub roots: Vec<FileId>,
    /// Files reached through literal SystemVerilog include directives. They are
    /// made available to slang through include buffers, but are not added
    /// as standalone semantic roots.
    pub include_only: FxHashSet<FileId>,
    pub include_dirs: Vec<AbsPathBuf>,
    pub top_modules: Vec<String>,
}

impl CompilationPlan {
    pub fn for_source_root(db: &dyn SourceRootDb, source_root_id: SourceRootId) -> Self {
        let project_config = db.project_config();
        let profile_id = project_config.profile_for_root(source_root_id);
        let (source_roots, top_modules, include_dirs) =
            profile_inputs(&project_config, Some(source_root_id), profile_id);
        Self::from_inputs(db, source_roots, top_modules, include_dirs)
    }

    pub fn for_profile(db: &dyn SourceRootDb, profile_id: Option<CompilationProfileId>) -> Self {
        let project_config = db.project_config();
        let (source_roots, top_modules, include_dirs) =
            profile_inputs(&project_config, None, profile_id);
        let source_roots =
            if source_roots.is_empty() { all_non_ignored_roots(db) } else { source_roots };
        Self::from_inputs(db, source_roots, top_modules, include_dirs)
    }

    fn from_inputs(
        db: &dyn SourceRootDb,
        source_roots: Vec<SourceRootId>,
        top_modules: Vec<String>,
        include_dirs: Vec<AbsPathBuf>,
    ) -> Self {
        let include_only = include_targets_for_source_roots(db, &source_roots, &include_dirs);
        let roots = compile_roots_for_source_roots(db, &source_roots, &include_only);
        CompilationPlan { source_roots, roots, include_only, include_dirs, top_modules }
    }
}

pub fn include_buffers_for_plan(
    db: &dyn SourceRootDb,
    plan: &CompilationPlan,
) -> Vec<SyntaxTreeBuffer> {
    include_buffers_for_plan_with_roots(db, plan, false)
}

pub fn compilation_source_buffers_for_plan(
    db: &dyn SourceRootDb,
    plan: &CompilationPlan,
) -> Vec<SyntaxTreeBuffer> {
    include_buffers_for_plan_with_roots(db, plan, true)
}

fn include_buffers_for_plan_with_roots(
    db: &dyn SourceRootDb,
    plan: &CompilationPlan,
    include_roots: bool,
) -> Vec<SyntaxTreeBuffer> {
    let root_files = include_roots
        .then(|| plan.roots.iter().copied().collect::<FxHashSet<_>>())
        .unwrap_or_default();
    let mut seen_files = PathIdentitySet::default();
    let mut seen_buffer_paths = FxHashSet::default();
    let mut buffers = Vec::new();

    for file_id in db.files().iter().copied() {
        if db.file_is_project_ignored(file_id) {
            continue;
        }

        let include_header_in_include_path =
            matches!(db.file_kind(file_id), SourceFileKind::IncludeHeader)
                && db.file_path(file_id).is_some_and(|path| {
                    plan.include_dirs.iter().any(|include_dir| path.starts_with(include_dir))
                });
        let semantic_root = root_files.contains(&file_id)
            && matches!(db.file_kind(file_id), SourceFileKind::SystemVerilog);
        if !semantic_root
            && !include_header_in_include_path
            && !plan.include_only.contains(&file_id)
        {
            continue;
        }

        let Some(path) = db.file_path(file_id) else {
            continue;
        };

        if !seen_files.insert_path(&path) {
            continue;
        }

        let path = path.to_string();
        if seen_buffer_paths.insert(path.clone()) {
            buffers.push(SyntaxTreeBuffer { path, text: db.file_text(file_id).to_string() });
        }
    }

    buffers
}

fn profile_inputs(
    project_config: &ProjectConfig,
    fallback_root: Option<SourceRootId>,
    profile_id: Option<CompilationProfileId>,
) -> (Vec<SourceRootId>, Vec<String>, Vec<AbsPathBuf>) {
    if let Some(profile) = profile_id.and_then(|profile_id| project_config.profile(profile_id)) {
        return (
            profile.source_roots.clone(),
            profile.top_modules.clone(),
            profile.preprocess.include_dirs.clone(),
        );
    }

    (
        fallback_root.into_iter().collect(),
        Vec::new(),
        project_config.preprocess_for_profile(profile_id).include_dirs,
    )
}

fn all_non_ignored_roots(db: &dyn SourceRootDb) -> Vec<SourceRootId> {
    let mut roots = FxHashSet::default();
    for file_id in db.files().iter().copied() {
        if !db.file_is_project_ignored(file_id) {
            roots.insert(db.source_root_id(file_id));
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

fn include_targets_for_source_roots(
    db: &dyn SourceRootDb,
    roots: &[SourceRootId],
    include_dirs: &[AbsPathBuf],
) -> FxHashSet<FileId> {
    let path_file_ids = path_file_ids(db);
    let mut included = FxHashSet::default();
    let mut scanned = FxHashSet::default();
    let mut pending = Vec::new();
    for root_id in roots {
        pending.extend(db.source_root(*root_id).iter());
    }

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

        let Some(includer_path) = db.file_path(file_id) else {
            continue;
        };

        for include in &db.preproc_file_index(file_id).includes {
            let MacroIncludeTarget::Literal { path, .. } = &include.target else {
                continue;
            };
            if let Some(included_file_id) =
                resolve_include_target(path, &includer_path, include_dirs, &path_file_ids)
                && included.insert(included_file_id)
            {
                pending.push(included_file_id);
            }
        }
    }

    included
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
