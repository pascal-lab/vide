pub mod macro_def;
pub mod project_manifest;
mod toml_workspace;

use anyhow::Context;
use base_db::source_root::SourceRootConfig;
use itertools::Itertools;
use utils::paths::AbsPathBuf;
use vfs::{FileSetConfig, VfsPath};

use crate::{project_manifest::ProjectManifest, toml_workspace::TomlWorkspace};

#[derive(Debug, PartialEq, Eq)]
pub struct Workspace(pub TomlWorkspace);

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct WorkspaceRoot {
    pub is_lib: bool,
    pub include: Vec<AbsPathBuf>,
    pub exclude: Vec<AbsPathBuf>,
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
            ProjectManifest::Discover(path) => Ok(Self(TomlWorkspace::default_from_path(path))),
        }
    }

    pub fn to_roots(&self) -> Vec<WorkspaceRoot> {
        let Workspace(TomlWorkspace { include, exclude, is_lib, .. }) = self;
        vec![WorkspaceRoot {
            is_lib: *is_lib,
            include: include.to_vec(),
            exclude: exclude.to_vec(),
        }]
    }
}

pub fn get_workspace_folder(
    workspaces: &[Workspace],
    global_excludes: &[AbsPathBuf],
) -> (Vec<vfs::loader::Entry>, Vec<usize>, SourceRootConfig) {
    let roots = workspaces.iter().flat_map(|ws| ws.to_roots()).collect_vec();

    let mut watch = Vec::new();
    let mut load = Vec::new();
    let mut fsc = FileSetConfig::builder();
    let mut local_filesets = Vec::new();

    for root in roots.into_iter().filter(|it| !it.include.is_empty()) {
        let root_file_set = root.include.iter().cloned().map(VfsPath::from).collect_vec();

        let entry = {
            let mut dirs = vfs::loader::Directories {
                extensions: ["v", "sv"].map(String::from).into(),
                include: root.include,
                exclude: root.exclude,
            };
            for excl in global_excludes {
                if dirs.include.iter().any(|incl| incl.starts_with(excl) || excl.starts_with(incl))
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

        if !root.is_lib {
            watch.push(load.len());
        }
        load.push(entry);
    }

    let fileset_config = fsc.build();

    (load, watch, SourceRootConfig { fileset_config, local_filesets })
}
