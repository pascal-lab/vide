//! LSP comparison harness: latency, slang compile ceiling, accuracy.
//!
//! Layout lives next to this file (`bench/`), not a `mod.rs`.

mod accuracy;
mod client;
mod measure;
mod report;
mod servers;
mod slang;
mod workloads;

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use anyhow::{Context, Result, bail};
use clap::Args;

use self::{
    accuracy::score_accuracy,
    measure::{MeasureConfig, measure_server},
    report::{BenchReport, write_report},
    servers::discover_servers,
    slang::measure_slang_compile,
    workloads::{OverlayGuard, Workload, load_catalog},
};

#[derive(Debug, Args)]
pub struct BenchArgs {
    /// Restrict to these workload names (default: every present submodule).
    #[arg(long)]
    pub workload: Vec<String>,
    /// Restrict to these servers: vide, slang-server, verible, svls.
    #[arg(long)]
    pub server: Vec<String>,
    /// Skip the slang compiler ceiling measurement.
    #[arg(long)]
    pub skip_slang: bool,
    /// Directory for JSON + Markdown (default: benches/results).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub fn run(workspace_root: &Path, args: BenchArgs) -> Result<()> {
    let catalog = load_catalog(workspace_root)?;
    let selected: Vec<&Workload> = if args.workload.is_empty() {
        catalog.iter().filter(|workload| workload.sources_present()).collect()
    } else {
        args.workload
            .iter()
            .map(|name| {
                catalog.iter().find(|workload| workload.name == *name).with_context(|| {
                    format!("unknown workload {name}; known: {}", catalog_names(&catalog))
                })
            })
            .collect::<Result<Vec<_>>>()?
    };
    if selected.is_empty() {
        bail!(
            "no workloads to run. Init a submodule, for example:\n  \
             git submodule update --init benches/workloads/common_cells"
        );
    }

    let servers = discover_servers(workspace_root, &args.server)?;
    if servers.is_empty() {
        bail!("no language servers found (expected at least a Vide binary)");
    }

    let out_dir = args.out.unwrap_or_else(|| workspace_root.join("benches/results"));
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    let stamp = timestamp();
    let mut report = BenchReport::new(workspace_root, &stamp);

    let measure_cfg = MeasureConfig::default();
    for workload in selected {
        if !workload.sources_present() {
            eprintln!(
                "skip {}: submodule not checked out at {}",
                workload.name,
                workload.path.display()
            );
            continue;
        }
        eprintln!("== {} ({}) — {} ==", workload.name, workload.size, workload.description);
        let _overlay = OverlayGuard::apply(workload)?;
        for server in &servers {
            eprintln!("  server {}", server.id);
            match measure_server(server, workload, &measure_cfg) {
                Ok(sample) => report.push_lsp(sample),
                Err(error) => {
                    eprintln!("    failed: {error:#}");
                    report.push_lsp_error(&workload.name, server.id, format!("{error:#}"));
                }
            }
        }
        if !args.skip_slang {
            match measure_slang_compile(workload) {
                Ok(sample) => report.push_slang(sample),
                Err(error) => eprintln!("  slang compiler skipped: {error:#}"),
            }
        }
        score_accuracy(&mut report, &workload.name);
    }

    let json_path = out_dir.join(format!("{stamp}.json"));
    let md_path = out_dir.join(format!("{stamp}.md"));
    write_report(&report, &json_path, &md_path)?;
    println!("{}", fs::read_to_string(&md_path)?);
    eprintln!("wrote {} and {}", json_path.display(), md_path.display());
    Ok(())
}

fn catalog_names(catalog: &[Workload]) -> String {
    catalog.iter().map(|workload| workload.name.as_str()).collect::<Vec<_>>().join(", ")
}

fn timestamp() -> String {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    format!("{}", now.as_secs())
}
