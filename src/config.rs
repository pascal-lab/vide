use crate::Opt;
use lsp_types::ClientCapabilities;
use project_model::project_manifest::ProjectManifest;
use serde::de::DeserializeOwned;
use serde_json::Error;
use std::{iter, path::PathBuf};
use utils::paths::AbsPathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub(crate) opt: Opt,
    pub(crate) workspace_roots: Vec<AbsPathBuf>,
    pub(crate) client_caps: lsp_types::ClientCapabilities,
    pub(crate) root_path: AbsPathBuf,
    pub(crate) user_config: UserConfig,
    pub(crate) detached_files: Vec<AbsPathBuf>,
    pub(crate) discovered_workspaces: Vec<ProjectManifest>,
}

#[derive(Debug, Clone)]
pub struct Snippet {}

#[derive(Debug, Clone)]
pub struct UserConfig {}

impl Default for UserConfig {
    fn default() -> Self {
        UserConfig {}
    }
}

impl Config {
    pub fn new(
        opt: Opt,
        root_path: AbsPathBuf,
        client_caps: ClientCapabilities,
        workspace_roots: Vec<AbsPathBuf>,
        user_config: UserConfig,
        detached_files: Vec<AbsPathBuf>,
        snippets: Vec<Snippet>,
    ) -> Self {
        let discovered_workspaces = Self::discover_workspaces(&workspace_roots);
        Config {
            opt,
            workspace_roots,
            client_caps,
            root_path,
            user_config,
            detached_files,
            discovered_workspaces,
        }
    }

    pub fn discover_workspaces(roots: &Vec<AbsPathBuf>) -> Vec<ProjectManifest> {
        let workspaces = ProjectManifest::discover_all(roots);
        tracing::info!("discovered workspaces: {workspaces:?}");
        if workspaces.is_empty() {
            tracing::info!("no workspaces discovered in {:?}", &roots);
        }
        return workspaces;
    }

    pub fn main_loop_threads_num(&self) -> usize {
        num_cpus::get_physical().try_into().unwrap_or(1)
    }
}

fn get_field<T: DeserializeOwned>(
    json: &mut serde_json::Value,
    error_sink: &mut Vec<(String, serde_json::Error)>,
    field: &'static str,
    alias: Option<&'static str>,
    default: &str,
) -> T {
    // check alias first, to work around the VS Code where it pre-fills the
    // defaults instead of sending an empty object.
    alias
        .into_iter()
        .chain(iter::once(field))
        .filter_map(move |field| {
            let mut pointer = field.replace('_', "/");
            pointer.insert(0, '/');
            json.pointer_mut(&pointer)
                .map(|it| serde_json::from_value(it.take()).map_err(|e| (e, pointer)))
        })
        .find(Result::is_ok)
        .and_then(|res| match res {
            Ok(it) => Some(it),
            Err((e, pointer)) => {
                tracing::warn!("Failed to deserialize config field at {}: {:?}", pointer, e);
                error_sink.push((pointer, e));
                None
            }
        })
        .unwrap_or_else(|| {
            serde_json::from_str(default).unwrap_or_else(|e| panic!("{e} on: `{default}`"))
        })
}

pub fn parse_initialization_options(
    mut options: serde_json::Value,
) -> (UserConfig, Vec<AbsPathBuf>, Vec<Snippet>, Vec<(String, Error)>) {
    tracing::info!("Server initialized with options: {:#}", options);
    if options.is_null() || options.as_object().map_or(false, |obj| obj.is_empty()) {
        return Default::default();
    }

    let mut errors = Vec::new();

    // TODO: user configuration in package.json
    let user_config: UserConfig = UserConfig {};

    let detached_files =
        get_field::<Vec<PathBuf>>(&mut options, &mut errors, "detachedFiles", None, "[]")
            .into_iter()
            .map(AbsPathBuf::assert)
            .collect::<Vec<AbsPathBuf>>();

    // TODO: user-defined snippets
    let snippets: Vec<Snippet> = Vec::new();

    (user_config, detached_files, snippets, errors)
}
