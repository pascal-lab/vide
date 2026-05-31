#![feature(try_blocks)]

use std::{env, path::PathBuf};

use clap::Parser;
use const_format::formatcp;
use itertools::Itertools;
use lsp_server::Connection;
use lsp_types::{MessageType, ShowMessageParams};
use slang as _;
use utils::{
    json::from_json,
    paths::{AbsPathBuf, patch_path_prefix},
};

use crate::{
    config::Config,
    global_state::main_loop,
    i18n::{I18n, Locale},
};

pub mod browser;
mod config;
mod global_state;
mod i18n;
mod lsp_ext;

#[cfg(feature = "user-config-schema")]
pub use config::user_config::{
    generated_user_config_schema, generated_vscode_configuration_typescript,
    generated_vscode_package_properties,
};

pub const DEFAULT_PROCESS_NAME: &str = env!("CARGO_PKG_NAME");
const DEBUG: bool = cfg!(debug_assertions);
const BUILD_PROFILE: &str = if DEBUG { "DEBUG" } else { "RELEASE" };
pub const VERSION: &str =
    formatcp!("{}_{}{}", env!("CARGO_PKG_VERSION"), BUILD_PROFILE, env!("VIDE_BUILD_METADATA"));

#[derive(Clone, Debug, Parser)]
#[clap(name = DEFAULT_PROCESS_NAME, version = VERSION)]
pub struct Opt {
    #[clap(long, default_value = DEFAULT_PROCESS_NAME)]
    pub process_name: String,

    #[clap(short, long, default_value = formatcp!("{}", if DEBUG { "debug" } else { "error" }))]
    pub log: String,

    #[clap(long = "log_file", default_value = None)]
    pub log_filename: Option<PathBuf>,

    /// Write a Chrome/Perfetto-compatible tracing profile to this JSON file.
    ///
    /// This can also be set with VIDE_PROFILE_TRACE. The captured targets
    /// default to project crates and can be overridden with
    /// VIDE_PROFILE_TRACE_FILTER.
    #[clap(long = "profile_trace", default_value = None)]
    pub profile_trace: Option<PathBuf>,
}

#[allow(deprecated)]
pub fn run_server(opt: Opt) -> anyhow::Result<()> {
    tracing::info!("Server {}_{} started.", &opt.process_name, VERSION);

    let (connection, io_threads) = Connection::stdio();
    let (initialize_id, initialize_params) = match connection.initialize_start() {
        Ok(it) => it,
        Err(e) => {
            if e.channel_is_disconnected() {
                io_threads.join()?;
            }
            return Err(e.into());
        }
    };

    tracing::info!("Server initialized. InitializeParams: {}", &initialize_params);

    let lsp_types::InitializeParams {
        root_uri,
        capabilities: client_caps,
        workspace_folders,
        initialization_options,
        trace,
        locale,
        ..
    } = from_json::<lsp_types::InitializeParams>("InitializeParams", &initialize_params)?;

    let root_path = match root_uri
        .and_then(|uri| uri.to_file_path().ok())
        .map(patch_path_prefix)
        .and_then(|path| AbsPathBuf::try_from(path).ok())
    {
        Some(path) => path,
        None => AbsPathBuf::try_from(env::current_dir()?).map_err(|path| {
            anyhow::format_err!(
                "current directory is not an absolute UTF-8 path: {}",
                path.display()
            )
        })?,
    };

    let workspace_roots = workspace_folders
        .map(|workspace| {
            workspace
                .into_iter()
                .filter_map(|folder| folder.uri.to_file_path().ok())
                .map(patch_path_prefix)
                .filter_map(|path| AbsPathBuf::try_from(path).ok())
                .collect_vec()
        })
        .filter(|folders| !folders.is_empty())
        .unwrap_or_else(|| vec![root_path.clone()]);

    let i18n = I18n::new(Locale::from_lsp(locale.as_deref()));

    let (user_config, snippets) = if let Some(options) = initialization_options {
        let (user_config, snippets, errors) = Config::parse_initialization_options(options);
        if !errors.is_empty() {
            use lsp_types::notification::{Notification, ShowMessage};
            let noti = lsp_server::Notification::new(
                ShowMessage::METHOD.to_string(),
                ShowMessageParams { typ: MessageType::WARNING, message: errors.message(i18n) },
            );
            if connection.sender.send(lsp_server::Message::Notification(noti)).is_err() {
                tracing::debug!(
                    "configuration warning dropped because client connection is closed"
                );
            }
        }
        (user_config, snippets)
    } else {
        Default::default()
    };

    let config =
        Config::new(opt, root_path, client_caps, workspace_roots, i18n, user_config, snippets);

    let initialize_result = lsp_types::InitializeResult {
        capabilities: config.server_caps(),
        server_info: Some(lsp_types::ServerInfo {
            name: config.opt.process_name.clone(),
            version: Some(VERSION.to_string()),
        }),
    };

    let initialize_result = serde_json::to_value(initialize_result)?;

    if let Err(e) = connection.initialize_finish(initialize_id, initialize_result) {
        if e.channel_is_disconnected() {
            io_threads.join()?;
        }
        return Err(e.into());
    }

    main_loop::main_loop(config, connection, trace.unwrap_or_default())?;

    io_threads.join()?;
    tracing::info!("Server shut down. BYE!");
    Ok(())
}

#[cfg(test)]
mod tests;
