pub mod caps;
pub mod user_config;

use std::fmt;

use itertools::Itertools;
use lsp_types::ClientCapabilities;
use project_model::project_manifest::ProjectManifest;
use utils::{
    lines::PositionEncoding,
    paths::{AbsPath, AbsPathBuf},
};

use self::user_config::{FilesWatcherDef, UserConfig};
use crate::Opt;

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

#[derive(Debug, Default)]
pub struct ConfigError {
    errors: Vec<(String, serde_json::Error)>,
}

impl std::error::Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors = self.errors.iter().format_with("\n", |(key, e), f| {
            f(key)?;
            f(&": ")?;
            f(e)
        });
        write!(
            f,
            "invalid config value{}:\n{}",
            if self.errors.len() == 1 { "" } else { "s" },
            errors
        )
    }
}

impl ConfigError {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) opt: Opt,
    pub(crate) workspace_roots: Vec<AbsPathBuf>,
    pub(crate) client_caps: lsp_types::ClientCapabilities,
    pub(crate) root_path: AbsPathBuf,
    pub(crate) user_config: UserConfig,
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
        _snippets: Vec<Snippet>,
    ) -> Self {
        let discovered_manifests = Self::discover_manifest(&workspace_roots);
        Config { opt, workspace_roots, client_caps, root_path, user_config, discovered_manifests }
    }

    pub(crate) fn update(&mut self, json: serde_json::Value) -> Result<(), ConfigError> {
        let (user_config, _snippets, errors) = Self::parse_initialization_options(json);
        self.user_config = user_config;

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn diagnostics_config(&self) -> base_db::diagnostics_config::DiagnosticsConfig {
        self.user_config.diagnostics_config()
    }

    pub(crate) fn parse_initialization_options(
        options: serde_json::Value,
    ) -> (UserConfig, Vec<Snippet>, ConfigError) {
        tracing::info!("Config updating from JSON: {:#}", options);
        if options.is_null() || options.as_object().is_some_and(|obj| obj.is_empty()) {
            return Default::default();
        }

        let mut errors = Vec::new();

        // TODO: user-defined snippets
        let snippets: Vec<Snippet> = Vec::new();

        let user_config = UserConfig::from_json(options, &mut errors);

        (user_config, snippets, ConfigError { errors })
    }

    fn discover_manifest(roots: &[AbsPathBuf]) -> Vec<ProjectManifest> {
        let manifests = ProjectManifest::discover_all(roots);
        tracing::info!("discovered manifests: {manifests:?}");
        if manifests.is_empty() {
            tracing::info!("no manifests discovered in {:?}", &roots);
        }
        manifests
    }

    pub fn main_loop_threads_num(&self) -> usize {
        num_cpus::get_physical()
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

    pub fn position_encoding(&self) -> PositionEncoding {
        self.negotiated_encoding()
    }

    pub fn remove_workspace(&mut self, path: &AbsPath) {
        if let Some(position) = self.workspace_roots.iter().position(|it| it == path) {
            self.workspace_roots.remove(position);
        }
    }

    pub fn add_workspaces(&mut self, paths: impl Iterator<Item = AbsPathBuf>) {
        self.workspace_roots.extend(paths);
    }

    pub fn rediscover_manifest(&mut self) {
        self.discovered_manifests = Self::discover_manifest(&self.workspace_roots);
    }
}
