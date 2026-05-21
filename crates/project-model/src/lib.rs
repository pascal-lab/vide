pub mod macro_def;
pub mod project_manifest;
mod toml_workspace;

use std::collections::VecDeque;

use anyhow::{Context, bail};
use base_db::{
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_root::{SourceRootConfig, SourceRootId, SourceRootRole},
};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
#[cfg(feature = "manifest-schema")]
pub use toml_workspace::{
    TOML_MANIFEST_SCHEMA_PATH, TOML_MANIFEST_SCHEMA_URL, TOML_MANIFEST_SCHEMA_VERSION,
    generated_toml_manifest_schema,
};
use triomphe::Arc;
use utils::paths::{AbsPathBuf, Utf8Component, Utf8Path, sort_and_remove_subfolders};
use vfs::{FileSetConfig, FileSetFilter, PathGlobMatcher, PathMatcher, VfsPath};

use crate::{
    macro_def::MacroDef, project_manifest::ProjectManifest, toml_workspace::TomlWorkspace,
};

const DEFAULT_INDEX_SOURCE_PATTERNS: &[&str] = &["**"];

#[derive(Debug, PartialEq, Eq)]
pub struct Workspace {
    workspace_root: AbsPathBuf,
    library_paths: Vec<AbsPathBuf>,
    kind: WorkspaceKind,
    roots: Vec<WorkspaceRoot>,
    semantic_profile: Option<WorkspaceSemanticProfile>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ProjectModel {
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct WorkspaceRoot {
    pub role: SourceRootRole,
    pub source: PathMatcher,
    pub include_dirs: Vec<AbsPathBuf>,
    pub exclude_globs: Option<PathGlobMatcher>,
    pub contributes_to_semantic_profile: bool,
}

impl WorkspaceRoot {
    pub fn load_paths(&self) -> Vec<AbsPathBuf> {
        let mut paths = self.include_dirs.clone();
        paths.extend(self.source.scan_roots().cloned());
        sort_and_remove_subfolders(&mut paths);
        paths
    }

    pub fn contributes_to_semantic_profile(&self) -> bool {
        self.contributes_to_semantic_profile
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkspaceKind {
    Local,
    Library,
}

impl WorkspaceKind {
    fn from_is_lib(is_lib: bool) -> Self {
        if is_lib { Self::Library } else { Self::Local }
    }

    fn is_library(self) -> bool {
        matches!(self, Self::Library)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceSemanticProfile {
    top_modules: Vec<String>,
    preprocess: PreprocessConfig,
}

enum ManifestSourcePolicy {
    /// `sources` was omitted, so load the workspace for best-effort navigation
    /// without using those scan roots as semantic include roots.
    DefaultIndex,
    /// `sources` was present, including the explicit empty-array case.
    Explicit(Vec<String>),
}

impl ManifestSourcePolicy {
    fn from_manifest(patterns: Option<Vec<String>>) -> Self {
        match patterns {
            Some(patterns) => Self::Explicit(patterns),
            None => Self::DefaultIndex,
        }
    }

    fn patterns(&self) -> Vec<String> {
        match self {
            Self::DefaultIndex => default_index_source_patterns(),
            Self::Explicit(patterns) => patterns.clone(),
        }
    }

    fn defaults_include_dirs_to_sources(&self) -> bool {
        matches!(self, Self::Explicit(_))
    }
}

impl Workspace {
    pub fn load(manifest: &ProjectManifest, is_lib: bool) -> anyhow::Result<Workspace> {
        Self::load_helper(manifest, is_lib)
            .with_context(|| format!("failed to load workspace {:?}", manifest))
    }

    fn load_helper(manifest: &ProjectManifest, is_lib: bool) -> anyhow::Result<Workspace> {
        match manifest {
            ProjectManifest::Toml(toml) => {
                let toml_workspace = TomlWorkspace::load_from_file(toml)
                    .with_context(|| "failed to load workspace in {manifest:?}")?;

                Self::from_toml(toml_workspace, is_lib)
            }
            ProjectManifest::UnconfiguredRoot(path) => {
                Ok(Self::from_unconfigured_root(path, is_lib))
            }
        }
    }

    fn from_toml(toml: TomlWorkspace, is_lib: bool) -> anyhow::Result<Self> {
        let TomlWorkspace {
            top_modules,
            workspace_root,
            macro_defs,
            source_patterns,
            include_dirs,
            libraries,
            exclude_patterns,
        } = toml;

        let kind = WorkspaceKind::from_is_lib(is_lib);
        let source_policy = ManifestSourcePolicy::from_manifest(source_patterns);
        let source_patterns = validate_manifest_patterns(source_policy.patterns(), "sources")?;
        let exclude_patterns = validate_manifest_patterns(exclude_patterns, "exclude")?;
        let exclude_globs = compile_manifest_globs(&workspace_root, exclude_patterns, "exclude")?;

        let mut source_paths = source_roots_for_patterns(&workspace_root, &source_patterns);
        sort_and_remove_subfolders(&mut source_paths);
        let source = compile_manifest_globs(&workspace_root, source_patterns, "sources")?
            .map_or_else(
                || PathMatcher::all_under_roots(Vec::new()),
                |matcher| PathMatcher::glob(source_paths.clone(), matcher),
            );

        let default_include_paths = if source_policy.defaults_include_dirs_to_sources() {
            source_paths.as_slice()
        } else {
            &[]
        };
        let include_dirs = resolve_include_dirs(include_dirs, default_include_paths);
        let library_paths = resolve_library_paths(libraries);
        let roots = workspace_roots(
            kind,
            &source_policy,
            !source_paths.is_empty(),
            source,
            include_dirs.clone(),
            exclude_globs,
        );
        let semantic_profile = roots
            .iter()
            .any(WorkspaceRoot::contributes_to_semantic_profile)
            .then(|| semantic_profile(top_modules, macro_defs, include_dirs));

        Ok(Self { workspace_root, library_paths, kind, roots, semantic_profile })
    }

    fn from_unconfigured_root(path: &AbsPathBuf, is_lib: bool) -> Self {
        let kind = WorkspaceKind::from_is_lib(is_lib);
        let source_roots = vec![path.clone()];
        let include_dirs = if kind.is_library() { source_roots.clone() } else { Vec::new() };
        let roots = workspace_roots(
            kind,
            &ManifestSourcePolicy::DefaultIndex,
            true,
            PathMatcher::all_under_roots(source_roots),
            include_dirs.clone(),
            None,
        );
        let semantic_profile = roots
            .iter()
            .any(WorkspaceRoot::contributes_to_semantic_profile)
            .then(|| semantic_profile(Vec::new(), MacroDef::default(), include_dirs));

        Self {
            workspace_root: path.clone(),
            library_paths: Vec::new(),
            kind,
            roots,
            semantic_profile,
        }
    }

    pub fn roots(&self) -> &[WorkspaceRoot] {
        &self.roots
    }

    fn semantic_profile(&self) -> Option<&WorkspaceSemanticProfile> {
        self.semantic_profile.as_ref()
    }

    fn root(&self) -> &AbsPathBuf {
        &self.workspace_root
    }

    fn library_paths(&self) -> &[AbsPathBuf] {
        &self.library_paths
    }

    pub fn is_lib(&self) -> bool {
        self.kind.is_library()
    }
}

fn semantic_profile(
    top_modules: Vec<String>,
    macro_defs: MacroDef,
    include_dirs: Vec<AbsPathBuf>,
) -> WorkspaceSemanticProfile {
    WorkspaceSemanticProfile {
        top_modules,
        preprocess: PreprocessConfig {
            predefines: macro_defs.to_predefine_strings(),
            include_dirs,
        },
    }
}

fn workspace_roots(
    kind: WorkspaceKind,
    source_policy: &ManifestSourcePolicy,
    has_source_paths: bool,
    source: PathMatcher,
    include_dirs: Vec<AbsPathBuf>,
    exclude_globs: Option<PathGlobMatcher>,
) -> Vec<WorkspaceRoot> {
    let mut roots = Vec::new();

    if kind.is_library() {
        push_workspace_root(
            &mut roots,
            SourceRootRole::Library,
            source,
            include_dirs,
            exclude_globs,
            true,
        );
        return roots;
    }

    match source_policy {
        ManifestSourcePolicy::DefaultIndex => {
            push_workspace_root(
                &mut roots,
                SourceRootRole::Local,
                PathMatcher::all_under_roots(Vec::new()),
                include_dirs,
                exclude_globs.clone(),
                true,
            );
            if has_source_paths {
                push_workspace_root(
                    &mut roots,
                    SourceRootRole::IndexOnly,
                    source,
                    Vec::new(),
                    exclude_globs,
                    false,
                );
            }
        }
        ManifestSourcePolicy::Explicit(_) => {
            push_workspace_root(
                &mut roots,
                SourceRootRole::Local,
                source,
                include_dirs,
                exclude_globs,
                true,
            );
        }
    }

    roots
}

fn push_workspace_root(
    roots: &mut Vec<WorkspaceRoot>,
    role: SourceRootRole,
    source: PathMatcher,
    include_dirs: Vec<AbsPathBuf>,
    exclude_globs: Option<PathGlobMatcher>,
    contributes_to_semantic_profile: bool,
) {
    let root = WorkspaceRoot {
        role,
        source,
        include_dirs,
        exclude_globs,
        contributes_to_semantic_profile,
    };
    if !root.load_paths().is_empty() {
        roots.push(root);
    }
}

fn default_index_source_patterns() -> Vec<String> {
    DEFAULT_INDEX_SOURCE_PATTERNS.iter().map(|pattern| (*pattern).to_owned()).collect()
}

fn resolve_include_dirs(
    configured: Option<Vec<AbsPathBuf>>,
    source_paths: &[AbsPathBuf],
) -> Vec<AbsPathBuf> {
    let mut include_dirs = configured.unwrap_or_else(|| source_paths.to_vec());
    sort_and_remove_subfolders(&mut include_dirs);
    include_dirs
}

fn resolve_library_paths(mut paths: Vec<AbsPathBuf>) -> Vec<AbsPathBuf> {
    sort_and_remove_subfolders(&mut paths);
    paths
}

fn validate_manifest_patterns(patterns: Vec<String>, field: &str) -> anyhow::Result<Vec<String>> {
    for pattern in &patterns {
        if pattern.is_empty() {
            bail!("manifest {field} glob pattern must not be empty");
        }
        if pattern.contains('\\') {
            bail!(
                "manifest {field} glob pattern {pattern:?} uses backslashes; use '/' as the path separator"
            );
        }

        let path = Utf8Path::new(pattern);
        if path.is_absolute() {
            bail!(
                "manifest {field} glob pattern {pattern:?} must be relative to the workspace root"
            );
        }
        if path.components().any(|component| {
            matches!(
                component,
                Utf8Component::Prefix(_) | Utf8Component::RootDir | Utf8Component::ParentDir
            )
        }) {
            bail!("manifest {field} glob pattern {pattern:?} must stay inside the workspace root");
        }
    }

    Ok(patterns)
}

fn compile_manifest_globs(
    workspace_root: &AbsPathBuf,
    patterns: Vec<String>,
    field: &str,
) -> anyhow::Result<Option<PathGlobMatcher>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    PathGlobMatcher::new(workspace_root.clone(), patterns)
        .map(Some)
        .with_context(|| format!("failed to compile manifest {field} glob patterns"))
}

fn source_roots_for_patterns(workspace_root: &AbsPathBuf, patterns: &[String]) -> Vec<AbsPathBuf> {
    patterns.iter().map(|pattern| source_root_for_pattern(workspace_root, pattern)).collect()
}

fn source_root_for_pattern(workspace_root: &AbsPathBuf, pattern: &str) -> AbsPathBuf {
    let root = literal_root_prefix(pattern);
    if root.is_empty() { workspace_root.clone() } else { workspace_root.absolutize(root) }
}

fn literal_root_prefix(pattern: &str) -> &str {
    let Some(meta_idx) = pattern.find(['*', '?', '[', '{']) else {
        return pattern.trim_end_matches('/');
    };

    let prefix = &pattern[..meta_idx];
    let trimmed = prefix.trim_end_matches('/');
    if prefix.ends_with('/') {
        return trimmed;
    }

    trimmed.rsplit_once('/').map_or("", |(parent, _)| parent)
}

impl ProjectModel {
    pub fn load(manifests: Vec<ProjectManifest>) -> (ProjectModel, Vec<anyhow::Error>) {
        let mut pending =
            manifests.into_iter().map(|manifest| (manifest, false)).collect::<VecDeque<_>>();
        let mut loaded_manifests = FxHashSet::default();
        let mut workspaces = Vec::new();
        let mut errors = Vec::new();

        while let Some((manifest, is_lib)) = pending.pop_front() {
            if !loaded_manifests.insert(manifest.clone()) {
                continue;
            }

            let workspace = match Workspace::load(&manifest, is_lib) {
                Ok(workspace) => workspace,
                Err(error) => {
                    errors.push(error);
                    continue;
                }
            };

            for package in workspace.library_paths() {
                match ProjectManifest::from_path(package) {
                    Ok(manifest) => {
                        if !loaded_manifests.contains(&manifest) {
                            pending.push_back((manifest, true));
                        }
                    }
                    Err(error) => errors.push(error),
                }
            }

            workspaces.push(workspace);
        }

        (ProjectModel { workspaces }, errors)
    }
}

pub fn get_workspace_folder(
    workspaces: &[Workspace],
    global_excludes: &[AbsPathBuf],
) -> (Vec<vfs::loader::Entry>, Vec<usize>, SourceRootConfig, Arc<ProjectConfig>) {
    let mut watch = Vec::new();
    let mut load = Vec::new();
    let mut fsc = FileSetConfig::builder();
    let mut fileset_roles = Vec::new();
    let mut root_workspaces = Vec::new();

    for (workspace_idx, workspace) in workspaces.iter().enumerate() {
        for root in workspace.roots() {
            let load_paths = root.load_paths();
            if load_paths.is_empty() {
                continue;
            }
            let root_file_set = load_paths.iter().cloned().map(VfsPath::from).collect_vec();
            let mut exclude_paths = Vec::new();
            for excl in global_excludes {
                if load_paths.iter().any(|incl| incl.starts_with(excl) || excl.starts_with(incl)) {
                    exclude_paths.push(excl.clone());
                }
            }
            let mut include = Vec::new();
            if !root.include_dirs.is_empty() {
                include.push(PathMatcher::all_under_roots(root.include_dirs.clone()));
            }
            if !root.source.is_empty() {
                include.push(root.source.clone());
            }
            let source =
                if root.source.is_empty() { Vec::new() } else { vec![root.source.clone()] };

            let entry = {
                let dirs = vfs::loader::Directories {
                    extensions: ["v", "sv", "vh", "svh", "svi", "map"].map(String::from).into(),
                    include: include.clone(),
                    exclude: exclude_paths.clone(),
                    exclude_globs: root.exclude_globs.clone(),
                };
                vfs::loader::Entry::Directories(dirs)
            };

            let root_idx = fsc.len();
            fileset_roles.push(root.role);

            fsc.add_filtered_file_set(
                root_file_set,
                FileSetFilter {
                    include,
                    source: Some(source),
                    exclude_paths,
                    exclude_globs: root.exclude_globs.clone(),
                },
            );

            if !root.role.is_library() {
                watch.push(load.len());
            }
            root_workspaces.push((root_idx, workspace_idx, root.contributes_to_semantic_profile()));
            load.push(entry);
        }
    }

    fileset_roles.push(SourceRootRole::Ignored);
    let source_root_count = fsc.len() + 1;
    let mut root_ids_by_workspace = FxHashMap::<usize, Vec<SourceRootId>>::default();
    for (root_idx, workspace_idx, contributes_to_semantic_profile) in &root_workspaces {
        if *contributes_to_semantic_profile {
            root_ids_by_workspace
                .entry(*workspace_idx)
                .or_default()
                .push(SourceRootId(*root_idx as u32));
        }
    }
    let dependency_roots_by_workspace =
        dependency_roots_by_workspace(workspaces, &root_ids_by_workspace);
    let mut root_profiles = vec![None; source_root_count];
    let mut profiles = Vec::new();

    for (root_idx, workspace_idx, contributes_to_semantic_profile) in root_workspaces {
        if !contributes_to_semantic_profile {
            continue;
        }
        let source_root_id = SourceRootId(root_idx as u32);
        let workspace = &workspaces[workspace_idx];
        let Some(profile) = workspace.semantic_profile() else {
            continue;
        };

        let profile_id = CompilationProfileId(profiles.len() as u32);
        root_profiles[root_idx] = Some(profile_id);

        let source_roots = std::iter::once(source_root_id)
            .chain(
                root_ids_by_workspace
                    .get(&workspace_idx)
                    .into_iter()
                    .flat_map(|roots| roots.iter().copied())
                    .filter(|root_id| *root_id != source_root_id),
            )
            .chain(
                dependency_roots_by_workspace
                    .get(&workspace_idx)
                    .into_iter()
                    .flat_map(|roots| roots.iter().copied()),
            )
            .unique()
            .collect();

        profiles.push(CompilationProfile {
            source_roots,
            top_modules: profile.top_modules.clone(),
            preprocess: profile.preprocess.clone(),
        });
    }

    let fileset_config = fsc.build();
    let project_config = Arc::new(ProjectConfig::new(root_profiles, profiles));

    (load, watch, SourceRootConfig { fileset_config, fileset_roles }, project_config)
}

fn dependency_roots_by_workspace(
    workspaces: &[Workspace],
    root_ids_by_workspace: &FxHashMap<usize, Vec<SourceRootId>>,
) -> FxHashMap<usize, Vec<SourceRootId>> {
    let mut dependencies = FxHashMap::default();
    for workspace_idx in 0..workspaces.len() {
        let mut seen = FxHashSet::default();
        let mut roots = Vec::new();
        collect_dependency_roots(
            workspaces,
            root_ids_by_workspace,
            workspace_idx,
            &mut seen,
            &mut roots,
        );
        dependencies.insert(workspace_idx, roots);
    }
    dependencies
}

fn collect_dependency_roots(
    workspaces: &[Workspace],
    root_ids_by_workspace: &FxHashMap<usize, Vec<SourceRootId>>,
    workspace_idx: usize,
    seen: &mut FxHashSet<usize>,
    roots: &mut Vec<SourceRootId>,
) {
    for package_path in workspaces[workspace_idx].library_paths() {
        for (candidate_idx, candidate) in workspaces.iter().enumerate() {
            if candidate_idx == workspace_idx
                || !candidate.root().starts_with(package_path)
                || !seen.insert(candidate_idx)
            {
                continue;
            }

            if let Some(root_ids) = root_ids_by_workspace.get(&candidate_idx) {
                roots.extend(root_ids.iter().copied());
            }
            collect_dependency_roots(workspaces, root_ids_by_workspace, candidate_idx, seen, roots);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use base_db::source_root::SourceRootRole;
    use utils::{lines::LineEnding, test_support::TestDir};
    use vfs::{Vfs, loader::LoadResult};

    use super::*;

    #[test]
    fn project_model_loads_external_package_as_library_once() {
        let base = TestDir::new("project-model-package");
        let root = base.join("root");
        let root_rtl = root.join("rtl");
        let package = base.join("pkg");
        let package_rtl = package.join("rtl");

        fs::create_dir_all(&root_rtl).unwrap();
        fs::create_dir_all(&package_rtl).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"top_modules = ["top"]
sources = ["rtl/**"]
libraries = ["../pkg"]
"#,
        )
        .unwrap();
        fs::write(package.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl/**"]"#)
            .unwrap();

        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 2);
        assert!(!model.workspaces[0].is_lib());
        assert!(model.workspaces[1].is_lib());
    }

    #[test]
    fn project_model_loads_unconfigured_external_package_as_library() {
        let base = TestDir::new("project-model-unconfigured-package");
        let root = base.join("root");
        let root_rtl = root.join("rtl");
        let package = base.join("pkg");
        let package_rtl = package.join("rtl");

        fs::create_dir_all(&root_rtl).unwrap();
        fs::create_dir_all(&package_rtl).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"top_modules = ["top"]
sources = ["rtl/**"]
libraries = ["../pkg/rtl"]
"#,
        )
        .unwrap();

        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 2);
        assert!(!model.workspaces[0].is_lib());
        assert!(model.workspaces[1].is_lib());

        let root_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let root_profile = project_config.profile(root_profile_id).unwrap();
        assert_eq!(root_profile.source_roots, vec![SourceRootId(0), SourceRootId(1)]);
    }

    #[test]
    fn unconfigured_root_has_no_compilation_profile() {
        let root = TestDir::new("project-model-unconfigured-root");
        fs::create_dir_all(root.join("rtl")).unwrap();
        let top = root.join("rtl/top.sv");

        let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, project_config) =
            get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
        assert_eq!(model.workspaces[0].roots()[0].role, SourceRootRole::IndexOnly);
        assert_eq!(
            source_root_config.fileset_roles,
            vec![SourceRootRole::IndexOnly, SourceRootRole::Ignored]
        );
        let dirs = match &load[0] {
            vfs::loader::Entry::Directories(dirs) => dirs,
            other => panic!("expected directory loader entry, got {other:?}"),
        };
        assert!(dirs.contains_file(top.as_path()));
        assert_eq!(project_config.profile_for_root(SourceRootId(0)), None);
    }

    #[test]
    fn empty_manifest_has_no_compilation_profile() {
        let root = TestDir::new("project-model-empty-manifest");
        fs::write(root.join(project_manifest::MANIFEST_FILE_NAME), "").unwrap();

        let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (_, _, source_root_config, project_config) =
            get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
        assert_eq!(model.workspaces[0].roots()[0].role, SourceRootRole::IndexOnly);
        assert_eq!(
            source_root_config.fileset_roles,
            vec![SourceRootRole::IndexOnly, SourceRootRole::Ignored]
        );
        assert_eq!(project_config.profile_for_root(SourceRootId(0)), None);
    }

    #[test]
    fn syntax_only_default_manifest_has_no_compilation_profile() {
        let root = TestDir::new("project-model-syntax-only-manifest");
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            "sources = []\ninclude_dirs = []\n",
        )
        .unwrap();

        let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, project_config) =
            get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
        assert!(model.workspaces[0].roots().is_empty());
        assert!(load.is_empty());
        assert_eq!(source_root_config.fileset_roles, vec![SourceRootRole::Ignored]);
        assert_eq!(project_config.profile_for_root(SourceRootId(0)), None);
    }

    #[test]
    fn headers_only_workspace_is_loaded() {
        let base = TestDir::new("project-model-headers-only");
        let root = base.join("root");
        let include = root.join("include");
        fs::create_dir_all(&include).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = []
include_dirs = ["include"]
"#,
        )
        .unwrap();

        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, _) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(load.len(), 1);
        assert_eq!(
            source_root_config.fileset_roles,
            vec![SourceRootRole::Local, SourceRootRole::Ignored]
        );
    }

    #[test]
    fn omitted_sources_with_include_dirs_keeps_default_index_out_of_profile() {
        let base = TestDir::new("project-model-default-index-with-include-dirs");
        let root = base.join("root");
        let include = root.join("include");
        let rtl = root.join("rtl");
        fs::create_dir_all(&include).unwrap();
        fs::create_dir_all(&rtl).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"include_dirs = ["include"]
"#,
        )
        .unwrap();

        let header = include.join("defs.svh");
        let top = rtl.join("top.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, project_config) =
            get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(load.len(), 2);
        assert_eq!(
            source_root_config.fileset_roles,
            vec![SourceRootRole::Local, SourceRootRole::IndexOnly, SourceRootRole::Ignored]
        );

        let include_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let include_profile = project_config.profile(include_profile_id).unwrap();
        assert_eq!(include_profile.source_roots, vec![SourceRootId(0)]);
        assert_eq!(project_config.profile_for_root(SourceRootId(1)), None);

        let mut vfs = Vfs::default();
        for file in [&header, &top] {
            vfs.set_file_contents(
                &VfsPath::from(file.clone()),
                LoadResult::Loaded(String::new(), LineEnding::Unix),
            );
        }

        let roots = source_root_config.partition(&vfs);
        assert_eq!(roots[0].role(), SourceRootRole::Local);
        assert!(roots[0].file_for_path(&VfsPath::from(header)).is_some());
        assert_eq!(roots[1].role(), SourceRootRole::IndexOnly);
        assert!(roots[1].file_for_path(&VfsPath::from(top)).is_some());
    }

    #[test]
    fn source_paths_default_include_dirs_to_sources() {
        let base = TestDir::new("project-model-source-path-default-includes");
        let root = base.join("root");
        let rtl = root.join("rtl");
        fs::create_dir_all(rtl.join("nested")).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
"#,
        )
        .unwrap();

        let top = rtl.join("nested/top.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        let dirs = match &load[0] {
            vfs::loader::Entry::Directories(dirs) => dirs,
            other => panic!("expected directory loader entry, got {other:?}"),
        };
        assert!(dirs.contains_file(top.as_path()));

        let profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let profile = project_config.profile(profile_id).unwrap();
        assert_eq!(profile.preprocess.include_dirs, [rtl]);
    }

    #[test]
    fn exclude_globs_filter_loaded_source_files() {
        let base = TestDir::new("project-model-excluded-source-root");
        let root = base.join("root");
        fs::create_dir_all(root.join("rtl")).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
exclude = ["rtl/**"]
"#,
        )
        .unwrap();

        let top = root.join("rtl/top.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(load.len(), 1);
        let dirs = match &load[0] {
            vfs::loader::Entry::Directories(dirs) => dirs,
            other => panic!("expected directory loader entry, got {other:?}"),
        };
        assert!(!dirs.contains_file(top.as_path()));
        assert!(project_config.profile_for_root(SourceRootId(0)).is_some());
    }

    #[test]
    fn source_and_exclude_globs_filter_loaded_and_open_files() {
        let base = TestDir::new("project-model-source-paths");
        let root = base.join("root");
        let rtl = root.join("rtl");
        let excluded = rtl.join("excluded");
        fs::create_dir_all(rtl.join("nested")).unwrap();
        fs::create_dir_all(&excluded).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
include_dirs = []
exclude = ["rtl/excluded/**"]
"#,
        )
        .unwrap();

        let top = rtl.join("nested/top.sv");
        let excluded_top = excluded.join("top.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, _) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(load.len(), 1);
        let dirs = match &load[0] {
            vfs::loader::Entry::Directories(dirs) => dirs,
            other => panic!("expected directory loader entry, got {other:?}"),
        };
        assert!(dirs.contains_file(top.as_path()));
        assert!(!dirs.contains_file(excluded_top.as_path()));

        let mut vfs = Vfs::default();
        for file in [&top, &excluded_top] {
            vfs.set_file_contents(
                &VfsPath::from(file.clone()),
                LoadResult::Loaded(String::new(), LineEnding::Unix),
            );
        }

        let roots = source_root_config.partition(&vfs);
        assert!(roots[0].file_for_path(&VfsPath::from(top)).is_some());
        assert_eq!(roots[1].role(), SourceRootRole::Ignored);
        assert!(roots[1].file_for_path(&VfsPath::from(excluded_top)).is_some());
    }

    #[test]
    fn source_globs_use_shell_separator_semantics() {
        let base = TestDir::new("project-model-source-globs");
        let root = base.join("root");
        let rtl = root.join("rtl");
        fs::create_dir_all(rtl.join("nested")).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/*.sv"]
include_dirs = []
"#,
        )
        .unwrap();

        let top = rtl.join("top.sv");
        let nested_top = rtl.join("nested/top.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, _, _) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        let dirs = match &load[0] {
            vfs::loader::Entry::Directories(dirs) => dirs,
            other => panic!("expected directory loader entry, got {other:?}"),
        };
        assert!(dirs.contains_file(top.as_path()));
        assert!(!dirs.contains_file(nested_top.as_path()));
    }

    #[test]
    fn exclude_globs_match_shell_patterns_across_source_roots() {
        let base = TestDir::new("project-model-exclude-globs");
        let root = base.join("root");
        let rtl = root.join("rtl");
        fs::create_dir_all(&rtl).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
include_dirs = []
exclude = ["**/*_bb.v"]
"#,
        )
        .unwrap();

        let top = rtl.join("top.sv");
        let blackbox = rtl.join("top_bb.v");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, _) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        let dirs = match &load[0] {
            vfs::loader::Entry::Directories(dirs) => dirs,
            other => panic!("expected directory loader entry, got {other:?}"),
        };
        assert!(dirs.contains_file(top.as_path()));
        assert!(!dirs.contains_file(blackbox.as_path()));

        let mut vfs = Vfs::default();
        for file in [&top, &blackbox] {
            vfs.set_file_contents(
                &VfsPath::from(file.clone()),
                LoadResult::Loaded(String::new(), LineEnding::Unix),
            );
        }

        let roots = source_root_config.partition(&vfs);
        assert!(roots[0].file_for_path(&VfsPath::from(top)).is_some());
        assert_eq!(roots[1].role(), SourceRootRole::Ignored);
        assert!(roots[1].file_for_path(&VfsPath::from(blackbox)).is_some());
    }

    #[test]
    fn manifest_globs_must_be_portable_workspace_relative_patterns() {
        let root = TestDir::new("project-model-invalid-globs");
        for (name, manifest_text) in [
            ("parent", "sources = [\"../rtl/**\"]\n"),
            ("backslash", "sources = [\"rtl\\\\**\"]\n"),
        ] {
            fs::write(root.join(project_manifest::MANIFEST_FILE_NAME), manifest_text).unwrap();

            let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();
            let (_model, errors) = ProjectModel::load(vec![manifest]);

            assert!(!errors.is_empty(), "{name} pattern should be rejected");
        }
    }

    #[test]
    fn workspace_profiles_include_only_dependency_library_roots() {
        let base = TestDir::new("project-model-dependency-closure");
        let root_a = base.join("root_a");
        let root_b = base.join("root_b");
        let pkg_a = base.join("pkg_a");
        let pkg_b = base.join("pkg_b");
        for dir in [&root_a, &root_b, &pkg_a, &pkg_b] {
            fs::create_dir_all(dir.join("rtl")).unwrap();
        }
        fs::write(
            root_a.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
libraries = ["../pkg_a"]
"#,
        )
        .unwrap();
        fs::write(
            root_b.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
libraries = ["../pkg_b"]
"#,
        )
        .unwrap();
        fs::write(pkg_a.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl/**"]"#)
            .unwrap();
        fs::write(pkg_b.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl/**"]"#)
            .unwrap();

        let (model, errors) = ProjectModel::load(vec![
            ProjectManifest::Toml(root_a.join(project_manifest::MANIFEST_FILE_NAME)),
            ProjectManifest::Toml(root_b.join(project_manifest::MANIFEST_FILE_NAME)),
        ]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        let root_a_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let root_b_profile_id = project_config.profile_for_root(SourceRootId(1)).unwrap();
        let root_a_profile = project_config.profile(root_a_profile_id).unwrap();
        let root_b_profile = project_config.profile(root_b_profile_id).unwrap();

        assert_eq!(root_a_profile.source_roots, vec![SourceRootId(0), SourceRootId(2)]);
        assert_eq!(root_b_profile.source_roots, vec![SourceRootId(1), SourceRootId(3)]);
    }

    #[test]
    fn workspace_profiles_include_transitive_shared_dependency_roots_once() {
        let base = TestDir::new("project-model-transitive-dependency-closure");
        let app = base.join("app");
        let lib_a = base.join("lib_a");
        let lib_b = base.join("lib_b");
        let common = base.join("common");
        for dir in [&app, &lib_a, &lib_b, &common] {
            fs::create_dir_all(dir.join("rtl")).unwrap();
        }
        fs::write(
            app.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
libraries = ["../lib_a", "../lib_b"]
"#,
        )
        .unwrap();
        fs::write(
            lib_a.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
libraries = ["../common"]
"#,
        )
        .unwrap();
        fs::write(
            lib_b.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
libraries = ["../common"]
"#,
        )
        .unwrap();
        fs::write(common.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl/**"]"#)
            .unwrap();

        let (model, errors) = ProjectModel::load(vec![ProjectManifest::Toml(
            app.join(project_manifest::MANIFEST_FILE_NAME),
        )]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 4);

        let app_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let app_profile = project_config.profile(app_profile_id).unwrap();

        assert_eq!(app_profile.source_roots.first().copied(), Some(SourceRootId(0)));
        assert_eq!(app_profile.source_roots.len(), 4);
        for dependency_root in [SourceRootId(1), SourceRootId(2), SourceRootId(3)] {
            assert!(
                app_profile.source_roots.contains(&dependency_root),
                "app profile should include dependency root {dependency_root:?}: {:?}",
                app_profile.source_roots
            );
        }
        assert_eq!(
            app_profile.source_roots.iter().filter(|root_id| **root_id == SourceRootId(3)).count(),
            1
        );
    }

    #[test]
    fn workspace_profile_includes_explicit_dependency_even_when_dependency_is_local() {
        let base = TestDir::new("project-model-local-dependency");
        let app = base.join("app");
        let pkg = base.join("pkg");
        fs::create_dir_all(app.join("rtl")).unwrap();
        fs::create_dir_all(pkg.join("rtl")).unwrap();
        fs::write(
            app.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
libraries = ["../pkg"]
"#,
        )
        .unwrap();
        fs::write(
            pkg.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/**"]
"#,
        )
        .unwrap();

        let (model, errors) = ProjectModel::load(vec![
            ProjectManifest::Toml(app.join(project_manifest::MANIFEST_FILE_NAME)),
            ProjectManifest::Toml(pkg.join(project_manifest::MANIFEST_FILE_NAME)),
        ]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 2);
        assert!(!model.workspaces[0].is_lib());
        assert!(!model.workspaces[1].is_lib());

        let app_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let app_profile = project_config.profile(app_profile_id).unwrap();
        assert_eq!(app_profile.source_roots, vec![SourceRootId(0), SourceRootId(1)]);
    }
}
