use std::{
    path::PathBuf,
    sync::Arc,
    fs,
    io,
};

use const_format::formatcp;
use anyhow::Context;
use clap::Parser;
use::anyhow;
use lsp_server::Connection;
use tracing_subscriber::{
    fmt::writer::BoxMakeWriter,
    Registry,
    filter::Targets,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

const DEFAULT_PROCESS_NAME: &str = "vizsla";
const DEBUG: bool = cfg!(debug_assertions);
const VERSION: &str = formatcp!("{}_{}",
                                        env!("CARGO_PKG_VERSION"),
                                        if DEBUG { "DEBUG" } else { "RELEASE" });

#[derive(Debug, Parser)]
#[clap(name = "vizsla")]
pub struct Opt {
    #[clap(long, default_value = DEFAULT_PROCESS_NAME)]
    pub process_name: String,

    #[clap(long, default_value = VERSION)]
    pub version: String,

    #[clap(short, long, default_value = "info")]
    pub log: String,

    #[clap(long = "log_file", default_value = None)]
    pub log_filename: Option<PathBuf>,
}

fn setup_logging(opt: &Opt) -> anyhow::Result<()> {
    let target: Targets = opt.log.parse()
                                 .with_context(|| format!("invalid log filter: `{}`", opt.log))?;

    let writer = match &opt.log_filename {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("could not create log directory: {}", parent.display()))?;
            }
            let file = fs::File::create(path)
                .with_context(|| format!("could not create log file: {}", path.display()))?;
            BoxMakeWriter::new(Arc::new(file))
        }
        None => BoxMakeWriter::new(io::stderr)
    };

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer);

    Registry::default()
        .with(target)
        .with(fmt_layer)
        .init();

    Ok(())
}

fn run_server(opt: &Opt) -> anyhow::Result<()> {
    tracing::info!("Server {}_{} started.", &opt.process_name, &opt.version);

    let (connection, io_threads) = Connection::stdio();
    let (initialize_id, initialize_params) = connection.initialize_start()?;

    tracing::info!("Server {}_{} initialized. InitializeParams: {}",
                   &opt.process_name,
                   &opt.version,
                   &initialize_params);

    tracing::info!("Server {}_{} shut down.", &opt.process_name, &opt.version);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    setup_logging(&opt)?;

    // TODO: start thead to run server
    run_server(&opt)?;

    Ok(())
}
