use std::{env, fs, io, path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::Parser;
use const_format::formatcp;
use itertools::Itertools;
use lsp_server::Connection;
use lsp_types::{MessageType, ShowMessageParams};
use tracing_subscriber::{
    filter::Targets, fmt::writer::BoxMakeWriter, layer::SubscriberExt, util::SubscriberInitExt,
    Registry,
};
use utils::{
    json::from_json,
    paths::{patch_path_prefix, AbsPathBuf},
};

mod caps;
mod config;
mod reload;

use config::Config;

mod dispatcher;
mod global_state;
mod line_idx;
mod lsp_ext;
mod main_loop;
mod mem_docs;

const DEFAULT_PROCESS_NAME: &str = "vizsla";
const DEBUG: bool = cfg!(debug_assertions);
const VERSION: &str =
    formatcp!("{}_{}", env!("CARGO_PKG_VERSION"), if DEBUG { "DEBUG" } else { "RELEASE" });

#[derive(Clone, Debug, Parser)]
#[clap(name = "vizsla", version = VERSION)]
pub struct Opt {
    #[clap(long, default_value = DEFAULT_PROCESS_NAME)]
    pub process_name: String,

    #[clap(short, long, default_value = "info")]
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
            BoxMakeWriter::new(Arc::new(file))
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
    let (initialize_id, initialize_params) = connection.initialize_start()?;
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
        None => {
            let cwd = env::current_dir()?;
            AbsPathBuf::assert(cwd)
        }
    };

    let workspace_roots = workspace_folders
        .map(|workspace| {
            workspace
                .into_iter()
                .filter_map(|folder| folder.uri.to_file_path().ok())
                .map(patch_path_prefix)
                .filter_map(|path| AbsPathBuf::try_from(path).ok())
                .collect::<Vec<_>>()
        })
        .filter(|folders| !folders.is_empty())
        .unwrap_or_else(|| vec![root_path.clone()]);

    let (user_config, detached_files, snippets) = initialization_options
        .map(|options| {
            let (user_config, detached_files, snippets, errors) =
                config::parse_initialization_options(options);
            if !errors.is_empty() {
                use lsp_types::notification::{Notification, ShowMessage};
                let errors_formatter = errors.iter().format_with("\n", |(key, err), f| {
                    let _ = f(key);
                    let _ = f(&": ");
                    f(err)
                });
                let noti = lsp_server::Notification::new(
                    ShowMessage::METHOD.to_string(),
                    ShowMessageParams {
                        typ: MessageType::WARNING,
                        message: format!("{}", errors_formatter),
                    },
                );
                connection.sender.send(lsp_server::Message::Notification(noti)).unwrap();
            }
            (user_config, detached_files, snippets)
        })
        .unwrap_or(Default::default());

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

    connection.initialize_finish(initialize_id, initialize_result)?;

    main_loop::main_loop(config, connection)?;

    io_threads.join()?;
    tracing::info!("Server shut down. BYE!");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    setup_logging(&opt)?;

    run_server(opt)?;

    Ok(())
}
