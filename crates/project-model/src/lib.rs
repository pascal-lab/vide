pub mod macro_def;
pub mod project_manifest;
mod toml_workspace;

use std::collections::VecDeque;

use anyhow::Context;
use base_db::{
    project::{CompilationProfile, CompilationProfileId, PreprocessConfig, ProjectConfig},
    source_root::{SourceRootConfig, SourceRootId},
};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
pub use toml_workspace::{
    TomlManifestDiagnostic, TomlManifestField, TomlManifestPath, toml_manifest_diagnostics,
    toml_manifest_field_at_offset, toml_manifest_fields, toml_manifest_path_at_offset,
};
use triomphe::Arc;
use utils::paths::{AbsPathBuf, sort_and_remove_subfolders};
use vfs::{FileSetConfig, VfsPath};

use crate::{project_manifest::ProjectManifest, toml_workspace::TomlWorkspace};

#[derive(Debug, PartialEq, Eq)]
pub struct Workspace(pub TomlWorkspace);

#[derive(Debug, PartialEq, Eq)]
pub struct ProjectModel {
    pub workspaces: Vec<Workspace>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct WorkspaceRoot {
    pub is_lib: bool,
    pub sources: Vec<AbsPathBuf>,
    pub include_dirs: Vec<AbsPathBuf>,
    pub exclude: Vec<AbsPathBuf>,
}

impl WorkspaceRoot {
    pub fn load_paths(&self) -> Vec<AbsPathBuf> {
        let mut paths = self.sources.clone();
        paths.extend(self.include_dirs.iter().cloned());
        sort_and_remove_subfolders(&mut paths);
        paths
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
                let toml_workspaces = TomlWorkspace::load_from_file(toml, is_lib)
                    .with_context(|| "failed to load workspace in {manifest:?}")?;

                Ok(Self(toml_workspaces))
            }
            ProjectManifest::UnconfiguredRoot(path) => {
                Ok(Self(TomlWorkspace::from_unconfigured_root(path, is_lib)))
            }
        }
    }

    pub fn to_roots(&self) -> Vec<WorkspaceRoot> {
        let Workspace(TomlWorkspace { sources, include_dirs, exclude, is_lib, .. }) = self;
        vec![WorkspaceRoot {
            is_lib: *is_lib,
            sources: sources.to_vec(),
            include_dirs: include_dirs.to_vec(),
            exclude: exclude.to_vec(),
        }]
    }

    fn top_modules(&self) -> Vec<String> {
        self.0.top_modules.clone()
    }

    fn root(&self) -> &AbsPathBuf {
        &self.0.workspace_root
    }

    fn library_paths(&self) -> &[AbsPathBuf] {
        &self.0.package
    }

    fn enables_semantic_diagnostics(&self) -> bool {
        self.0.configures_semantic_diagnostics || self.0.is_lib
    }

    fn preprocess_config(&self) -> PreprocessConfig {
        PreprocessConfig {
            predefines: self.0.macro_defs.to_predefine_strings(),
            include_dirs: self.0.include_dirs.clone(),
        }
    }
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

            for package in &workspace.0.package {
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
    let mut local_filesets = Vec::new();
    let mut root_workspaces = Vec::new();
    let mut source_paths_by_fileset = Vec::new();

    for (workspace_idx, workspace) in workspaces.iter().enumerate() {
        for root in workspace.to_roots() {
            let load_paths = root.load_paths();
            if load_paths.is_empty() {
                continue;
            }
            let root_file_set = load_paths.iter().cloned().map(VfsPath::from).collect_vec();

            let entry = {
                let mut dirs = vfs::loader::Directories {
                    extensions: ["v", "sv", "vh", "svh", "svi", "map"].map(String::from).into(),
                    include: load_paths,
                    exclude: root.exclude,
                };
                for excl in global_excludes {
                    if dirs
                        .include
                        .iter()
                        .any(|incl| incl.starts_with(excl) || excl.starts_with(incl))
                    {
                        dirs.exclude.push(excl.clone());
                    }
                }

                vfs::loader::Entry::Directories(dirs)
            };

            if !root.is_lib {
                local_filesets.push(fsc.len());
            }

            fsc.add_file_set(root_file_set);
            source_paths_by_fileset.push(root.sources);

            if !root.is_lib {
                watch.push(load.len());
            }
            root_workspaces.push((fsc.len() - 1, workspace_idx, root.is_lib));
            load.push(entry);
        }
    }

    let ignored_filesets = vec![fsc.len()];
    let source_root_count = fsc.len() + 1;
    let root_id_by_workspace = root_workspaces
        .iter()
        .map(|(root_idx, workspace_idx, _)| (*workspace_idx, SourceRootId(*root_idx as u32)))
        .collect::<FxHashMap<_, _>>();
    let dependency_roots_by_workspace =
        dependency_roots_by_workspace(workspaces, &root_id_by_workspace);
    let mut root_profiles = vec![None; source_root_count];
    let mut profiles = Vec::new();

    for (root_idx, workspace_idx, _is_lib) in root_workspaces {
        let source_root_id = SourceRootId(root_idx as u32);
        let workspace = &workspaces[workspace_idx];
        if !workspace.enables_semantic_diagnostics() {
            continue;
        }

        let profile_id = CompilationProfileId(profiles.len() as u32);
        root_profiles[root_idx] = Some(profile_id);

        let source_roots = std::iter::once(source_root_id)
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
            top_modules: workspace.top_modules(),
            preprocess: workspace.preprocess_config(),
        });
    }

    let fileset_config = fsc.build();
    let project_config = Arc::new(ProjectConfig::new(root_profiles, profiles));

    (
        load,
        watch,
        SourceRootConfig {
            fileset_config,
            local_filesets,
            ignored_filesets,
            source_paths_by_fileset,
        },
        project_config,
    )
}

fn dependency_roots_by_workspace(
    workspaces: &[Workspace],
    root_id_by_workspace: &FxHashMap<usize, SourceRootId>,
) -> FxHashMap<usize, Vec<SourceRootId>> {
    let mut dependencies = FxHashMap::default();
    for workspace_idx in 0..workspaces.len() {
        let mut seen = FxHashSet::default();
        let mut roots = Vec::new();
        collect_dependency_roots(
            workspaces,
            root_id_by_workspace,
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
    root_id_by_workspace: &FxHashMap<usize, SourceRootId>,
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

            if let Some(root_id) = root_id_by_workspace.get(&candidate_idx).copied() {
                roots.push(root_id);
            }
            collect_dependency_roots(workspaces, root_id_by_workspace, candidate_idx, seen, roots);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use utils::test_support::TestDir;

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
sources = ["rtl"]
libraries = ["../pkg"]
"#,
        )
        .unwrap();
        fs::write(package.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl"]"#)
            .unwrap();

        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 2);
        assert!(!model.workspaces[0].0.is_lib);
        assert!(model.workspaces[1].0.is_lib);
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
sources = ["rtl"]
libraries = ["../pkg/rtl"]
"#,
        )
        .unwrap();

        let manifest = ProjectManifest::from_path(&root).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 2);
        assert!(!model.workspaces[0].0.is_lib);
        assert!(model.workspaces[1].0.is_lib);

        let root_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let root_profile = project_config.profile(root_profile_id).unwrap();
        assert_eq!(root_profile.source_roots, vec![SourceRootId(0), SourceRootId(1)]);
    }

    #[test]
    fn unconfigured_root_has_no_compilation_profile() {
        let root = TestDir::new("project-model-unconfigured-root");
        fs::create_dir_all(root.join("rtl")).unwrap();

        let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
        assert_eq!(project_config.profile_for_root(SourceRootId(0)), None);
    }

    #[test]
    fn empty_manifest_has_no_compilation_profile() {
        let root = TestDir::new("project-model-empty-manifest");
        fs::write(root.join(project_manifest::MANIFEST_FILE_NAME), "").unwrap();

        let manifest = ProjectManifest::from_path(&root.path().to_path_buf()).unwrap();
        let (model, errors) = ProjectModel::load(vec![manifest]);
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
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
        let (_, _, _, project_config) = get_workspace_folder(&model.workspaces, &[]);

        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(model.workspaces.len(), 1);
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
        assert_eq!(source_root_config.ignored_filesets, vec![1]);
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
            r#"sources = ["rtl"]
libraries = ["../pkg_a"]
"#,
        )
        .unwrap();
        fs::write(
            root_b.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl"]
libraries = ["../pkg_b"]
"#,
        )
        .unwrap();
        fs::write(pkg_a.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl"]"#)
            .unwrap();
        fs::write(pkg_b.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl"]"#)
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
            r#"sources = ["rtl"]
libraries = ["../lib_a", "../lib_b"]
"#,
        )
        .unwrap();
        fs::write(
            lib_a.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl"]
libraries = ["../common"]
"#,
        )
        .unwrap();
        fs::write(
            lib_b.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl"]
libraries = ["../common"]
"#,
        )
        .unwrap();
        fs::write(common.join(project_manifest::MANIFEST_FILE_NAME), r#"sources = ["rtl"]"#)
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
            r#"sources = ["rtl"]
libraries = ["../pkg"]
"#,
        )
        .unwrap();
        fs::write(
            pkg.join(project_manifest::MANIFEST_FILE_NAME),
            r#"sources = ["rtl"]
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
        assert!(!model.workspaces[0].0.is_lib);
        assert!(!model.workspaces[1].0.is_lib);

        let app_profile_id = project_config.profile_for_root(SourceRootId(0)).unwrap();
        let app_profile = project_config.profile(app_profile_id).unwrap();
        assert_eq!(app_profile.source_roots, vec![SourceRootId(0), SourceRootId(1)]);
    }
}
