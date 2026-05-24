pub mod caps;
pub mod user_config;

use std::fmt;

use base_db::diagnostics_config::DiagnosticsConfig;
use itertools::Itertools;
use lsp_types::ClientCapabilities;
use project_model::project_manifest::ProjectManifest;
use utils::{
    lines::PositionEncoding,
    paths::{AbsPath, AbsPathBuf},
};

use self::user_config::{FilesWatcherDef, UserConfig};
use crate::{
    Opt,
    i18n::{I18n, keys},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesConfig {
    pub watcher: FilesWatcher,
    pub exclude: Vec<AbsPathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
        write!(
            f,
            "invalid config value{}:\n{}",
            if self.errors.len() == 1 { "" } else { "s" },
            self.error_lines()
        )
    }
}

impl ConfigError {
    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub(crate) fn message(&self, i18n: I18n) -> String {
        let key = if self.errors.len() == 1 {
            keys::CONFIG_INVALID_VALUE_ONE
        } else {
            keys::CONFIG_INVALID_VALUE_MANY
        };
        i18n.format(key, [("errors", self.error_lines())])
    }

    fn error_lines(&self) -> String {
        self.errors
            .iter()
            .format_with("\n", |(key, e), f| {
                f(key)?;
                f(&": ")?;
                f(e)
            })
            .to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) opt: Opt,
    pub(crate) workspace_roots: Vec<AbsPathBuf>,
    pub(crate) client_caps: lsp_types::ClientCapabilities,
    pub(crate) root_path: AbsPathBuf,
    pub(crate) i18n: I18n,
    pub(crate) user_config: UserConfig,
    diagnostics_config: DiagnosticsConfig,
    pub(crate) project_manifests: Vec<ProjectManifest>,
}

#[derive(Debug, Clone)]
pub struct Snippet {}

impl Config {
    pub(crate) fn new(
        opt: Opt,
        root_path: AbsPathBuf,
        client_caps: ClientCapabilities,
        workspace_roots: Vec<AbsPathBuf>,
        i18n: I18n,
        user_config: UserConfig,
        _snippets: Vec<Snippet>,
    ) -> Self {
        let project_manifests = Self::project_manifests(&workspace_roots);
        let diagnostics_config = user_config.diagnostics_config().with_fresh_revision();
        Config {
            opt,
            workspace_roots,
            client_caps,
            root_path,
            i18n,
            user_config,
            diagnostics_config,
            project_manifests,
        }
    }

    pub(crate) fn update(&mut self, json: serde_json::Value) -> Result<(), ConfigError> {
        let (user_config, _snippets, errors) = Self::parse_initialization_options(json);
        let diagnostics_config = self.updated_diagnostics_config(&user_config);
        self.user_config = user_config;
        self.diagnostics_config = diagnostics_config;

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn diagnostics_config(&self) -> base_db::diagnostics_config::DiagnosticsConfig {
        self.diagnostics_config.clone()
    }

    fn updated_diagnostics_config(&self, user_config: &UserConfig) -> DiagnosticsConfig {
        let mut next = user_config.diagnostics_config();
        if next.has_same_settings(&self.diagnostics_config) {
            next.revision = self.diagnostics_config.revision;
            next
        } else {
            next.with_fresh_revision()
        }
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

    fn project_manifests(roots: &[AbsPathBuf]) -> Vec<ProjectManifest> {
        let (manifests, errors) = ProjectManifest::from_paths(roots);
        for error in errors {
            tracing::error!("failed to resolve project path: {error:#}");
        }
        tracing::info!("project manifests: {manifests:?}");
        if manifests.is_empty() {
            tracing::info!("no project paths resolved in {:?}", &roots);
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

    pub(crate) fn workspace_affecting_settings_changed(&self, other: &Config) -> bool {
        self.files() != other.files()
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

    pub fn refresh_project_manifests(&mut self) {
        self.project_manifests = Self::project_manifests(&self.workspace_roots);
    }
}
