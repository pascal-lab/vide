#![feature(let_chains)]
#![feature(try_blocks)]
use std::{env, fs, io, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use config::Config;
use const_format::formatcp;
use itertools::Itertools;
use lsp_server::Connection;
use lsp_types::{MessageType, ShowMessageParams};
use tracing_subscriber::{
    Registry, filter::Targets, fmt::writer::BoxMakeWriter, layer::SubscriberExt,
    util::SubscriberInitExt,
};
use triomphe::Arc;
use utils::{
    json::from_json,
    paths::{AbsPathBuf, patch_path_prefix},
};

use crate::global_state::main_loop;

mod config;
mod global_state;
mod lsp_ext;

const DEFAULT_PROCESS_NAME: &str = env!("CARGO_PKG_NAME");
const DEBUG: bool = cfg!(debug_assertions);
const VERSION: &str =
    formatcp!("{}_{}", env!("CARGO_PKG_VERSION"), if DEBUG { "DEBUG" } else { "RELEASE" });

#[derive(Clone, Debug, Parser)]
#[clap(name = DEFAULT_PROCESS_NAME, version = VERSION)]
pub struct Opt {
    #[clap(long, default_value = DEFAULT_PROCESS_NAME)]
    pub process_name: String,

    #[clap(short, long, default_value = formatcp!("{}", if DEBUG { "info" } else { "error" }))]
    pub log: String,

    #[clap(long = "log_file", default_value = None)]
    pub log_filename: Option<PathBuf>,
}

fn setup_logging(opt: &Opt) -> anyhow::Result<()> {
    let target: Targets =
        opt.log.parse().with_context(|| format!("invalid log filter: `{}`", opt.log))?;

    let writer = match &opt.log_filename {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("could not create log directory: {}", parent.display())
                })?;
            }
            let file = fs::File::create(path)
                .with_context(|| format!("could not create log file: {}", path.display()))?;
            BoxMakeWriter::new(std::sync::Arc::new(file))
        }
        None => BoxMakeWriter::new(io::stderr),
    };

    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(writer);

    Registry::default().with(target).with(fmt_layer).init();

    Ok(())
}

fn run_server(opt: Opt) -> anyhow::Result<()> {
    tracing::info!("Server {}_{} started.", &opt.process_name, VERSION);

    // Start connection
    let (connection, io_threads) = Connection::stdio();

    // Initialize server
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
        ..
    } = from_json::<lsp_types::InitializeParams>("InitializeParams", &initialize_params)?;

    let root_path = match root_uri
        .and_then(|uri| uri.to_file_path().ok())
        .map(patch_path_prefix)
        .and_then(|path| AbsPathBuf::try_from(path).ok())
    {
        Some(path) => path,
        None => AbsPathBuf::assert_utf8(env::current_dir()?),
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

    let (user_config, detached_files, snippets) = if initialization_options.is_some() {
        let (user_config, detached_files, snippets, errors) =
            Config::parse_initialization_options(initialization_options.unwrap());
        if !errors.is_empty() {
            use lsp_types::notification::{Notification, ShowMessage};
            let noti =
                lsp_server::Notification::new(ShowMessage::METHOD.to_string(), ShowMessageParams {
                    typ: MessageType::WARNING,
                    message: errors.to_string(),
                });
            connection.sender.send(lsp_server::Message::Notification(noti)).unwrap();
        }
        (user_config, Arc::new(detached_files), snippets)
    } else {
        Default::default()
    };

    let config = Config::new(
        opt,
        root_path,
        client_caps,
        workspace_roots,
        user_config,
        detached_files,
        snippets,
    );

    let initialize_result = lsp_types::InitializeResult {
        capabilities: config.get_server_capabilities(),
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

    main_loop::main_loop(config, connection)?;

    io_threads.join()?;
    tracing::info!("Server shut down. BYE!");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        env::set_var("RUST_BACKTRACE", "short");
    }

    let opt = Opt::parse();
    setup_logging(&opt)?;
    run_server(opt)?;
    Ok(())
}
