use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::Parser;
use tracing_subscriber::{
    Layer, Registry, filter::Targets, fmt::writer::BoxMakeWriter, layer::SubscriberExt,
    util::SubscriberInitExt,
};
use vide::{Opt, run_server};

const DEFAULT_PROFILE_TRACE_FILTER: &str = concat!(
    "vide=trace,",
    "hir::base_db=trace,",
    "hir=trace,",
    "ide=trace,",
    "project_model=trace,",
    "slang=trace,",
    "utils=trace,",
    "vfs=trace,",
    "vfs::notify=trace"
);

fn profile_trace_path(opt: &Opt) -> Option<PathBuf> {
    opt.profile_trace.clone().or_else(|| env::var_os("VIDE_PROFILE_TRACE").map(PathBuf::from))
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
        let profile_filter_text = env::var("VIDE_PROFILE_TRACE_FILTER")
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
