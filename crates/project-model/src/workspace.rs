use anyhow::Context;
use triomphe::Arc;
use utils::paths::AbsPathBuf;

use crate::{project_manifest::ProjectManifest, toml_workspace::TomlWorkspace};

#[derive(Debug, PartialEq, Eq)]
pub enum Workspace {
    Project(TomlWorkspace),
    DetachedFiles(Arc<Vec<AbsPathBuf>>),
}

impl Workspace {
    pub fn load(manifest: &ProjectManifest, is_lib: bool) -> anyhow::Result<Workspace> {
        Self::load_helper(&manifest, is_lib)
            .with_context(|| format!("failed to load workspace {:?}", &manifest))
    }

    fn load_helper(manifest: &ProjectManifest, is_lib: bool) -> anyhow::Result<Workspace> {
        match manifest {
            ProjectManifest::Toml(toml) => {
                assert_eq!(toml.extension().unwrap(), "toml");

                let toml_workspaces = TomlWorkspace::load_from_file(toml, is_lib)
                    .with_context(|| "failed to load workspace in {manifest:?}")?;

                Ok(Workspace::Project(toml_workspaces))
            }
            ProjectManifest::Discover(path) => {
                Ok(Workspace::Project(TomlWorkspace::default_from_path(path)))
            }
        }
    }

    pub fn load_detached_files(files: Arc<Vec<AbsPathBuf>>) -> anyhow::Result<Workspace> {
        Ok(Workspace::DetachedFiles(files))
    }

    pub fn to_roots(&self) -> Vec<AbsPathBuf> {
        todo!()
    }
}
