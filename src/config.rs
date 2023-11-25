use crate::{
    user_config::{FilesWatcherDef, UserConfig},
    Opt,
};
use itertools::Itertools;
use lsp_types::ClientCapabilities;
use project_model::project_manifest::ProjectManifest;
use serde_json::Error;
use std::path::PathBuf;
use triomphe::Arc;
use utils::{json::get_field, paths::AbsPathBuf};

#[derive(Debug, Clone)]
pub struct FilesConfig {
    pub watcher: FilesWatcher,
    pub exclude: Vec<AbsPathBuf>,
}

#[derive(Debug, Clone)]
pub enum FilesWatcher {
    Client,
    Server,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) opt: Opt,
    pub(crate) workspace_roots: Vec<AbsPathBuf>,
    pub(crate) client_caps: lsp_types::ClientCapabilities,
    pub(crate) root_path: AbsPathBuf,
    pub(crate) user_config: UserConfig,
    pub(crate) detached_files: Arc<Vec<AbsPathBuf>>,
    pub(crate) discovered_manifests: Vec<ProjectManifest>,
}

#[derive(Debug, Clone)]
pub struct Snippet {}

impl Config {
    pub(crate) fn new(
        opt: Opt,
        root_path: AbsPathBuf,
        client_caps: ClientCapabilities,
        workspace_roots: Vec<AbsPathBuf>,
        user_config: UserConfig,
        detached_files: Arc<Vec<AbsPathBuf>>,
        snippets: Vec<Snippet>,
    ) -> Self {
        let discovered_manifests = Self::discover_manifest(&workspace_roots);
        Config {
            opt,
            workspace_roots,
            client_caps,
            root_path,
            user_config,
            detached_files,
            discovered_manifests,
        }
    }

    pub(crate) fn parse_initialization_options(
        mut options: serde_json::Value,
    ) -> (UserConfig, Vec<AbsPathBuf>, Vec<Snippet>, Vec<(String, Error)>) {
        tracing::info!("Server initialized with options: {:#}", options);
        if options.is_null() || options.as_object().map_or(false, |obj| obj.is_empty()) {
            return Default::default();
        }

        let mut errors = Vec::new();

        let detached_files =
            get_field::<Vec<PathBuf>>(&mut options, &mut errors, "detachedFiles", "[]")
                .into_iter()
                .map(AbsPathBuf::assert)
                .collect_vec();

        // TODO: user-defined snippets
        let snippets: Vec<Snippet> = Vec::new();

        let user_config = UserConfig::from_json(options, &mut errors);

        (user_config, detached_files, snippets, errors)
    }

    pub fn discover_manifest(roots: &Vec<AbsPathBuf>) -> Vec<ProjectManifest> {
        let manifests = ProjectManifest::discover_all(roots);
        tracing::info!("discovered manifests: {manifests:?}");
        if manifests.is_empty() {
            tracing::info!("no manifests discovered in {:?}", &roots);
        }
        return manifests;
    }

    pub fn main_loop_threads_num(&self) -> usize {
        num_cpus::get_physical().try_into().unwrap_or(1)
    }

    pub fn files(&self) -> FilesConfig {
        FilesConfig {
            watcher: match self.user_config.files_watcher {
                FilesWatcherDef::Client if self.cli_did_change_watched_files_dyn_reg() => {
                    FilesWatcher::Client
                }
                _ => FilesWatcher::Server,
            },
            exclude: self
                .user_config
                .files_excludeDirs
                .iter()
                .map(|it| self.root_path.join(it))
                .collect(),
        }
    }
}
