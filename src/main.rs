#![feature(try_blocks)]
use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use config::Config;
use const_format::formatcp;
use itertools::Itertools;
use lsp_server::Connection;
use lsp_types::{MessageType, ShowMessageParams};
use slang as _;
use tracing_subscriber::{
    Layer, Registry, filter::Targets, fmt::writer::BoxMakeWriter, layer::SubscriberExt,
    util::SubscriberInitExt,
};
use utils::{
    json::from_json,
    paths::{AbsPathBuf, patch_path_prefix},
};

use crate::{
    global_state::main_loop,
    i18n::{I18n, Locale},
};

mod config;
mod global_state;
mod i18n;
mod lsp_ext;

const DEFAULT_PROCESS_NAME: &str = env!("CARGO_PKG_NAME");
const DEBUG: bool = cfg!(debug_assertions);
const BUILD_PROFILE: &str = if DEBUG { "DEBUG" } else { "RELEASE" };
const VERSION: &str = formatcp!(
    "{}_{}+{}.{}",
    env!("CARGO_PKG_VERSION"),
    BUILD_PROFILE,
    env!("VIZSLA_COMMIT_HASH"),
    env!("VIZSLA_BUILD_DATE")
);
const DEFAULT_PROFILE_TRACE_FILTER: &str = concat!(
    "vizsla=trace,",
    "base_db=trace,",
    "hir=trace,",
    "ide=trace,",
    "project_model=trace,",
    "slang=trace,",
    "utils=trace,",
    "vfs=trace,",
    "vfs_notify=trace"
);

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
    /// This can also be set with VIZSLA_PROFILE_TRACE. The captured targets
    /// default to project crates and can be overridden with
    /// VIZSLA_PROFILE_TRACE_FILTER.
    #[clap(long = "profile_trace", default_value = None)]
    pub profile_trace: Option<PathBuf>,
}

fn profile_trace_path(opt: &Opt) -> Option<PathBuf> {
    opt.profile_trace.clone().or_else(|| env::var_os("VIZSLA_PROFILE_TRACE").map(PathBuf::from))
}

fn create_profile_trace_file(path: &Path) -> anyhow::Result<fs::File> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("could not create profile trace directory: {}", parent.display())
        })?;
    }
    fs::File::create(path)
        .with_context(|| format!("could not create profile trace file: {}", path.display()))
}

fn setup_logging(opt: &Opt) -> anyhow::Result<Option<tracing_chrome::FlushGuard>> {
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

    let fmt_layer =
        tracing_subscriber::fmt::layer().with_ansi(false).with_writer(writer).with_filter(target);

    let subscriber = Registry::default().with(fmt_layer);
    let profile_guard = if let Some(path) = profile_trace_path(opt) {
        let profile_filter_text = env::var("VIZSLA_PROFILE_TRACE_FILTER")
            .unwrap_or_else(|_| DEFAULT_PROFILE_TRACE_FILTER.to_owned());
        let profile_filter =
            profile_filter_text.parse::<Targets>().context("invalid profile trace filter")?;
        let file = create_profile_trace_file(&path)?;
        let (chrome_layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .writer(file)
            .include_args(true)
            .include_locations(false)
            .build();
        subscriber.with(chrome_layer.with_filter(profile_filter)).init();
        tracing::info!(
            path = %path.display(),
            filter = %profile_filter_text,
            "profile trace enabled"
        );
        Some(guard)
    } else {
        subscriber.init();
        None
    };

    Ok(profile_guard)
}

#[allow(deprecated)]
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

fn main() -> anyhow::Result<()> {
    if env::var("RUST_BACKTRACE").is_err() {
        unsafe {
            env::set_var("RUST_BACKTRACE", "short");
        }
    }

    let opt = Opt::parse();
    let _profile_guard = setup_logging(&opt)?;
    run_server(opt)?;
    Ok(())
}

#[cfg(test)]
mod tests;
