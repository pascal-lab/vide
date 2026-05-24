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
use utils::{
    path_identity::PathIdentitySet,
    paths::{AbsPathBuf, Utf8Component, Utf8Path, sort_and_remove_subfolders},
};
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

/// A project-model root after manifest policy has been applied.
///
/// The fields are split by consumer. `source` classifies VFS files into source
/// roots, `source_directories` drives recursive loader/watch scans, and
/// `source_files` carries exact manifest files through
/// [`vfs::loader::Entry::Files`].
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct WorkspaceRoot {
    pub role: SourceRootRole,
    /// Files matching this are semantic source files for this root.
    pub source: PathMatcher,
    /// Source globs that are safe to expand as directories.
    pub source_directories: PathMatcher,
    /// Literal source files from the manifest.
    pub source_files: Vec<AbsPathBuf>,
    /// Include/search roots loaded as headers and passed to preprocessing.
    pub include_dirs: Vec<AbsPathBuf>,
    pub exclude_globs: Option<PathGlobMatcher>,
}

impl WorkspaceRoot {
    /// Paths used to build the file-set prefix map.
    ///
    /// This includes exact source-file paths so a manually opened file can be
    /// classified without requiring its parent directory to be a load root.
    pub fn file_set_paths(&self) -> Vec<AbsPathBuf> {
        let mut paths = self.include_dirs.clone();
        paths.extend(self.source.scan_roots().cloned());
        sort_and_remove_subfolders(&mut paths);
        paths
    }

    /// Roots that should be recursively expanded by the loader or client
    /// watcher.
    ///
    /// Exact source files are intentionally excluded and are handled through
    /// `source_files`.
    pub fn directory_load_paths(&self) -> Vec<AbsPathBuf> {
        let mut paths = self.include_dirs.clone();
        paths.extend(self.source_directories.scan_roots().cloned());
        sort_and_remove_subfolders(&mut paths);
        paths
    }

    fn has_load_paths(&self) -> bool {
        !self.source_files.is_empty()
            || !self.include_dirs.is_empty()
            || !self.source_directories.is_empty()
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

        let source_locations = source_locations_for_patterns(&workspace_root, &source_patterns);
        let source = compile_manifest_globs(&workspace_root, source_patterns.clone(), "sources")?
            .map_or_else(
                || PathMatcher::all_under_roots(Vec::new()),
                |matcher| PathMatcher::glob(source_locations.matcher_roots.clone(), matcher),
            );
        let has_source_paths = source_locations.has_load_paths();
        let source_directories = compile_manifest_globs(
            &workspace_root,
            source_locations.directory_patterns,
            "sources",
        )?
        .map_or_else(
            || PathMatcher::all_under_roots(Vec::new()),
            |matcher| PathMatcher::glob(source_locations.directory_roots.clone(), matcher),
        );

        let default_include_paths = if source_policy.defaults_include_dirs_to_sources() {
            source_locations.default_include_dirs.as_slice()
        } else {
            &[]
        };
        let include_dirs = resolve_include_dirs(include_dirs, default_include_paths);
        let library_paths = resolve_library_paths(libraries);
        let root_parts = WorkspaceRootParts {
            source,
            source_directories,
            source_files: source_locations.source_files,
            include_dirs: include_dirs.clone(),
            exclude_globs,
        };
        let roots = workspace_roots(kind, &source_policy, has_source_paths, root_parts);
        let semantic_profile = roots
            .iter()
            .any(|root| root.role.participates_in_semantic_profile())
            .then(|| semantic_profile(top_modules, macro_defs, include_dirs));

        Ok(Self { workspace_root, library_paths, kind, roots, semantic_profile })
    }

    fn from_unconfigured_root(path: &AbsPathBuf, is_lib: bool) -> Self {
        let kind = WorkspaceKind::from_is_lib(is_lib);
        let source_roots = vec![path.clone()];
        let include_dirs = if kind.is_library() { source_roots.clone() } else { Vec::new() };
        let source = PathMatcher::all_under_roots(source_roots.clone());
        let root_parts = WorkspaceRootParts {
            source: source.clone(),
            source_directories: source,
            source_files: Vec::new(),
            include_dirs: include_dirs.clone(),
            exclude_globs: None,
        };
        let roots = workspace_roots(kind, &ManifestSourcePolicy::DefaultIndex, true, root_parts);
        let semantic_profile = roots
            .iter()
            .any(|root| root.role.participates_in_semantic_profile())
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

/// Root ingredients before default-source policy splits them into separate
/// local and best-effort roots.
#[derive(Clone)]
struct WorkspaceRootParts {
    source: PathMatcher,
    source_directories: PathMatcher,
    source_files: Vec<AbsPathBuf>,
    include_dirs: Vec<AbsPathBuf>,
    exclude_globs: Option<PathGlobMatcher>,
}

impl WorkspaceRootParts {
    fn include_only(&self) -> Self {
        Self {
            source: PathMatcher::all_under_roots(Vec::new()),
            source_directories: PathMatcher::all_under_roots(Vec::new()),
            source_files: Vec::new(),
            include_dirs: self.include_dirs.clone(),
            exclude_globs: self.exclude_globs.clone(),
        }
    }

    fn source_only(&self) -> Self {
        Self {
            source: self.source.clone(),
            source_directories: self.source_directories.clone(),
            source_files: self.source_files.clone(),
            include_dirs: Vec::new(),
            exclude_globs: self.exclude_globs.clone(),
        }
    }
}

fn workspace_roots(
    kind: WorkspaceKind,
    source_policy: &ManifestSourcePolicy,
    has_source_paths: bool,
    parts: WorkspaceRootParts,
) -> Vec<WorkspaceRoot> {
    let mut roots = Vec::new();

    if kind.is_library() {
        push_workspace_root(&mut roots, SourceRootRole::Library, parts);
        return roots;
    }

    match source_policy {
        ManifestSourcePolicy::DefaultIndex => {
            push_workspace_root(&mut roots, SourceRootRole::Local, parts.include_only());
            if has_source_paths {
                push_workspace_root(
                    &mut roots,
                    SourceRootRole::BestEffortIndex,
                    parts.source_only(),
                );
            }
        }
        ManifestSourcePolicy::Explicit(_) => {
            push_workspace_root(&mut roots, SourceRootRole::Local, parts);
        }
    }

    roots
}

fn push_workspace_root(
    roots: &mut Vec<WorkspaceRoot>,
    role: SourceRootRole,
    parts: WorkspaceRootParts,
) {
    let root = WorkspaceRoot {
        role,
        source: parts.source,
        source_directories: parts.source_directories,
        source_files: parts.source_files,
        include_dirs: parts.include_dirs,
        exclude_globs: parts.exclude_globs,
    };
    if root.has_load_paths() {
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

/// Normalized manifest source-pattern facts, separated by how each consumer
/// uses them.
#[derive(Debug, Default)]
struct SourceLocations {
    matcher_roots: Vec<AbsPathBuf>,
    directory_roots: Vec<AbsPathBuf>,
    directory_patterns: Vec<String>,
    source_files: Vec<AbsPathBuf>,
    default_include_dirs: Vec<AbsPathBuf>,
}

impl SourceLocations {
    fn finish(mut self) -> Self {
        sort_and_remove_subfolders(&mut self.matcher_roots);
        sort_and_remove_subfolders(&mut self.directory_roots);
        sort_and_remove_subfolders(&mut self.default_include_dirs);
        self.source_files.sort();
        self.source_files.dedup();
        self
    }

    fn has_load_paths(&self) -> bool {
        !self.directory_roots.is_empty() || !self.source_files.is_empty()
    }
}

fn source_locations_for_patterns(
    workspace_root: &AbsPathBuf,
    patterns: &[String],
) -> SourceLocations {
    let mut locations = SourceLocations::default();
    for pattern in patterns {
        if let Some(source_file) = literal_source_file(workspace_root, pattern) {
            locations.matcher_roots.push(source_file.clone());
            locations.source_files.push(source_file.clone());
            locations.default_include_dirs.push(
                source_file
                    .parent()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| workspace_root.clone()),
            );
            continue;
        }

        let root = literal_root_prefix(pattern);
        let source_root =
            if root.is_empty() { workspace_root.clone() } else { workspace_root.absolutize(root) };
        locations.matcher_roots.push(source_root.clone());
        locations.directory_roots.push(source_root.clone());
        locations.default_include_dirs.push(source_root);
        locations.directory_patterns.push(pattern.clone());
    }

    locations.finish()
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

fn literal_source_file(workspace_root: &AbsPathBuf, pattern: &str) -> Option<AbsPathBuf> {
    if pattern.ends_with('/') || pattern.find(['*', '?', '[', '{']).is_some() {
        return None;
    }

    let manifest_path = Utf8Path::new(pattern);
    let absolute_path = workspace_root.absolutize(manifest_path);
    if let Ok(metadata) = std::fs::metadata(absolute_path.as_path()) {
        return metadata.is_file().then_some(absolute_path);
    }

    if manifest_path.extension().is_some_and(|ext| {
        vfs::loader::SOURCE_FILE_EXTENSIONS
            .iter()
            .any(|candidate| ext.eq_ignore_ascii_case(candidate))
    }) {
        return Some(absolute_path);
    }

    None
}

impl ProjectModel {
    pub fn load(manifests: Vec<ProjectManifest>) -> (ProjectModel, Vec<anyhow::Error>) {
        let mut pending =
            manifests.into_iter().map(|manifest| (manifest, false)).collect::<VecDeque<_>>();
        let mut loaded_manifests = ProjectManifestIdentitySet::default();
        let mut workspaces = Vec::new();
        let mut errors = Vec::new();

        while let Some((manifest, is_lib)) = pending.pop_front() {
            if !loaded_manifests.insert(&manifest) {
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
                        pending.push_back((manifest, true));
                    }
                    Err(error) => errors.push(error),
                }
            }

            workspaces.push(workspace);
        }

        (ProjectModel { workspaces }, errors)
    }
}

#[derive(Default)]
struct ProjectManifestIdentitySet {
    paths: PathIdentitySet,
}

impl ProjectManifestIdentitySet {
    fn insert(&mut self, manifest: &ProjectManifest) -> bool {
        let path = match manifest {
            ProjectManifest::Toml(path) | ProjectManifest::UnconfiguredRoot(path) => path,
        };
        self.paths.insert_path(path.as_path())
    }
}

struct LoadedWorkspaceRoot {
    root_idx: usize,
    workspace_idx: usize,
    role: SourceRootRole,
}

pub fn get_workspace_folder(
    workspaces: &[Workspace],
    global_excludes: &[AbsPathBuf],
) -> (Vec<vfs::loader::Entry>, Vec<usize>, SourceRootConfig, Arc<ProjectConfig>) {
    let mut watch = Vec::new();
    let mut load = Vec::new();
    let mut fsc = FileSetConfig::builder();
    let mut fileset_roles = Vec::new();
    let mut loaded_roots = Vec::new();

    for (workspace_idx, workspace) in workspaces.iter().enumerate() {
        for root in workspace.roots() {
            if !root.has_load_paths() {
                continue;
            }
            let file_set_paths = root.file_set_paths();
            let root_file_set = file_set_paths.iter().cloned().map(VfsPath::from).collect_vec();
            let mut exclude_paths = Vec::new();
            for excl in global_excludes {
                if file_set_paths
                    .iter()
                    .any(|incl| incl.starts_with(excl) || excl.starts_with(incl))
                {
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

            let mut load_entries = Vec::new();
            let source_files = root
                .source_files
                .iter()
                .filter(|path| {
                    !is_excluded_load_file(path.as_path(), &exclude_paths, &root.exclude_globs)
                })
                .cloned()
                .collect_vec();
            if !source_files.is_empty() {
                load_entries.push(vfs::loader::Entry::Files(source_files));
            }

            let mut directory_include = Vec::new();
            if !root.include_dirs.is_empty() {
                directory_include.push(PathMatcher::all_under_roots(root.include_dirs.clone()));
            }
            if !root.source_directories.is_empty() {
                directory_include.push(root.source_directories.clone());
            }
            if !directory_include.is_empty() {
                let dirs = vfs::loader::Directories {
                    extensions: source_file_extensions(),
                    include: directory_include,
                    exclude: exclude_paths.clone(),
                    exclude_globs: root.exclude_globs.clone(),
                };
                load_entries.push(vfs::loader::Entry::Directories(dirs));
            }

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

            loaded_roots.push(LoadedWorkspaceRoot { root_idx, workspace_idx, role: root.role });
            for entry in load_entries {
                if root.role.is_watched() {
                    watch.push(load.len());
                }
                load.push(entry);
            }
        }
    }

    fileset_roles.push(SourceRootRole::Ignored);
    let source_root_count = fsc.len() + 1;
    let mut root_ids_by_workspace = FxHashMap::<usize, Vec<SourceRootId>>::default();
    for loaded_root in &loaded_roots {
        if loaded_root.role.participates_in_semantic_profile() {
            root_ids_by_workspace
                .entry(loaded_root.workspace_idx)
                .or_default()
                .push(SourceRootId(loaded_root.root_idx as u32));
        }
    }
    let dependency_roots_by_workspace =
        dependency_roots_by_workspace(workspaces, &root_ids_by_workspace);
    let mut root_profiles = vec![None; source_root_count];
    let mut profiles = Vec::new();

    for loaded_root in loaded_roots {
        if !loaded_root.role.participates_in_semantic_profile() {
            continue;
        }
        let source_root_id = SourceRootId(loaded_root.root_idx as u32);
        let workspace = &workspaces[loaded_root.workspace_idx];
        let Some(profile) = workspace.semantic_profile() else {
            continue;
        };

        let profile_id = CompilationProfileId(profiles.len() as u32);
        root_profiles[loaded_root.root_idx] = Some(profile_id);

        let source_roots = std::iter::once(source_root_id)
            .chain(
                root_ids_by_workspace
                    .get(&loaded_root.workspace_idx)
                    .into_iter()
                    .flat_map(|roots| roots.iter().copied())
                    .filter(|root_id| *root_id != source_root_id),
            )
            .chain(
                dependency_roots_by_workspace
                    .get(&loaded_root.workspace_idx)
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

fn source_file_extensions() -> Vec<String> {
    vfs::loader::SOURCE_FILE_EXTENSIONS.iter().map(|ext| (*ext).to_owned()).collect()
}

fn is_excluded_load_file(
    path: &utils::paths::AbsPath,
    exclude_paths: &[AbsPathBuf],
    exclude_globs: &Option<PathGlobMatcher>,
) -> bool {
    exclude_paths.iter().any(|exclude| path.starts_with(exclude))
        || exclude_globs.as_ref().is_some_and(|exclude| exclude.is_match(path))
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

    use base_db::{source_db::SourceFileKind, source_root::SourceRootRole};
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

    #[cfg(windows)]
    #[test]
    fn project_model_deduplicates_manifest_path_identities() {
        let root = TestDir::new("project-model-manifest-identity");
        fs::create_dir_all(root.join("rtl")).unwrap();
        let manifest_path = root.join(project_manifest::MANIFEST_FILE_NAME);
        fs::write(&manifest_path, r#"sources = ["rtl/**"]"#).unwrap();
        let verbatim_manifest_path =
            AbsPathBuf::try_from(format!(r"\\?\{manifest_path}").as_str()).unwrap();

        let (model, errors) = ProjectModel::load(vec![
            ProjectManifest::Toml(manifest_path),
            ProjectManifest::Toml(verbatim_manifest_path),
        ]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
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
        assert_eq!(model.workspaces[0].roots()[0].role, SourceRootRole::BestEffortIndex);
        assert_eq!(
            source_root_config.fileset_roles,
            vec![SourceRootRole::BestEffortIndex, SourceRootRole::Ignored]
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
        assert_eq!(model.workspaces[0].roots()[0].role, SourceRootRole::BestEffortIndex);
        assert_eq!(
            source_root_config.fileset_roles,
            vec![SourceRootRole::BestEffortIndex, SourceRootRole::Ignored]
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
            vec![SourceRootRole::Local, SourceRootRole::BestEffortIndex, SourceRootRole::Ignored]
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
        assert_eq!(roots[1].role(), SourceRootRole::BestEffortIndex);
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
    fn explicit_single_file_source_separates_file_load_from_include_scan_root() {
        let base = TestDir::new("project-model-single-file-source");
        let root = base.join("root");
        let rtl = root.join("rtl");
        fs::create_dir_all(&rtl).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/top.sv"]
"#,
        )
        .unwrap();

        let top = rtl.join("top.sv");
        let sibling = rtl.join("sibling.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, project_config) =
            get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert!(load.iter().any(|entry| match entry {
            vfs::loader::Entry::Files(files) => files == std::slice::from_ref(&top),
            vfs::loader::Entry::Directories(_) => false,
        }));
        let dirs = load
            .iter()
            .find_map(|entry| match entry {
                vfs::loader::Entry::Directories(dirs) => Some(dirs),
                vfs::loader::Entry::Files(_) => None,
            })
            .expect("default include dir should add a directory loader entry");
        assert!(dirs.contains_file(sibling.as_path()));

        let profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let profile = project_config.profile(profile_id).unwrap();
        assert_eq!(profile.preprocess.include_dirs, [rtl]);

        let mut vfs = Vfs::default();
        for file in [&top, &sibling] {
            vfs.set_file_contents(
                &VfsPath::from(file.clone()),
                LoadResult::Loaded(String::new(), LineEnding::Unix),
            );
        }

        let roots = source_root_config.partition(&vfs);
        let top_file_id = *roots[0].file_for_path(&VfsPath::from(top)).unwrap();
        let sibling_file_id = *roots[0].file_for_path(&VfsPath::from(sibling)).unwrap();
        assert_eq!(roots[0].file_kind(&top_file_id), SourceFileKind::SystemVerilog);
        assert_eq!(roots[0].file_kind(&sibling_file_id), SourceFileKind::IncludeHeader);
    }

    #[test]
    fn explicit_single_file_source_without_include_dirs_loads_only_that_file() {
        let base = TestDir::new("project-model-single-file-source-no-includes");
        let root = base.join("root");
        let rtl = root.join("rtl");
        fs::create_dir_all(&rtl).unwrap();
        fs::write(
            root.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl/top.sv"]
include_dirs = []
"#,
        )
        .unwrap();

        let top = rtl.join("top.sv");
        let sibling = rtl.join("sibling.sv");
        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (load, _, source_root_config, _) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(load.len(), 1);
        assert!(
            matches!(&load[0], vfs::loader::Entry::Files(files) if files == std::slice::from_ref(&top))
        );

        let mut vfs = Vfs::default();
        for file in [&top, &sibling] {
            vfs.set_file_contents(
                &VfsPath::from(file.clone()),
                LoadResult::Loaded(String::new(), LineEnding::Unix),
            );
        }

        let roots = source_root_config.partition(&vfs);
        assert!(roots[0].file_for_path(&VfsPath::from(top)).is_some());
        assert_eq!(roots[1].role(), SourceRootRole::Ignored);
        assert!(roots[1].file_for_path(&VfsPath::from(sibling)).is_some());
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
