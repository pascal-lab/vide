use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::{measure::LspSample, slang::SlangSample};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccuracyRow {
    pub workload: String,
    pub server: String,
    pub probe: String,
    pub method: String,
    pub kind: String,
    pub matched: usize,
    pub extra: usize,
    pub missing: usize,
    pub oracle_count: usize,
    pub got_count: usize,
    pub nonempty: bool,
    pub oracle_nonempty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    pub commit: String,
    pub generated_unix: String,
    pub lsp: Vec<LspSample>,
    pub slang: Vec<SlangSample>,
    pub accuracy: Vec<AccuracyRow>,
    pub notes: Vec<String>,
}

impl BenchReport {
    pub fn new(workspace_root: &Path, stamp: &str) -> Self {
        Self {
            commit: git_head(workspace_root),
            generated_unix: stamp.to_owned(),
            lsp: Vec::new(),
            slang: Vec::new(),
            accuracy: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn push_lsp(&mut self, sample: LspSample) {
        self.lsp.push(sample);
    }

    pub fn push_lsp_error(&mut self, workload: &str, server: &str, error: String) {
        self.lsp.push(LspSample {
            workload: workload.to_owned(),
            size: String::new(),
            server: server.to_owned(),
            oracle: false,
            initialize_ms: 0,
            ready_ms: None,
            rss_kb: None,
            requests: Vec::new(),
            error: Some(error),
        });
    }

    pub fn push_slang(&mut self, sample: SlangSample) {
        self.slang.push(sample);
    }
}

pub fn write_report(report: &BenchReport, json_path: &Path, md_path: &Path) -> Result<()> {
    fs::write(json_path, serde_json::to_string_pretty(report)?)
        .with_context(|| format!("failed to write {}", json_path.display()))?;
    fs::write(md_path, render_markdown(report))
        .with_context(|| format!("failed to write {}", md_path.display()))?;
    Ok(())
}

fn render_markdown(report: &BenchReport) -> String {
    let mut out = String::new();
    out.push_str("# Vide comparison report\n\n");
    out.push_str(&format!(
        "commit `{}` · generated `{}`\n\n",
        report.commit, report.generated_unix
    ));
    out.push_str("Latency is wall-clock milliseconds of the LSP request. `ready` is how long after `initialize` the server first resolved the workload's ready position; every `cold` below is measured after that, so it times a real answer rather than a server that is still indexing. `warm` is p50/p95 of 10 repeats after the first hit. `after-edit` is the next request after a body-only append. slang-server is the accuracy oracle. The `slang` compiler row is a full-compile ceiling, not an LSP.\n\n");

    out.push_str("## LSP latency\n\n");
    out.push_str("| workload | size | server | init | ready | rss | probe | method | cold | warm p50/p95 | after-edit |\n");
    out.push_str("| --- | --- | --- | ---: | ---: | ---: | --- | --- | ---: | ---: | ---: |\n");
    for sample in &report.lsp {
        if let Some(error) = &sample.error {
            out.push_str(&format!(
                "| {} | {} | {} | — | — | — | — | — | failed: {} |\n",
                sample.workload, sample.size, sample.server, error
            ));
            continue;
        }
        let rss = sample.rss_kb.map(|kb| format!("{} KB", kb)).unwrap_or_else(|| "—".into());
        let ready = sample.ready_ms.map(|ms| ms.to_string()).unwrap_or_else(|| "—".into());
        if sample.requests.is_empty() {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {ready} | {rss} | — | — | — | — | — |\n",
                sample.workload, sample.size, sample.server, sample.initialize_ms
            ));
            continue;
        }
        for request in &sample.requests {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {ready} | {rss} | {} | {} | {} | {}/{} | {} |\n",
                sample.workload,
                sample.size,
                sample.server,
                sample.initialize_ms,
                request.probe,
                short_method(&request.method),
                request.cold_ms,
                request.warm_p50_ms,
                request.warm_p95_ms,
                request.after_edit_ms
            ));
        }
    }

    if !report.slang.is_empty() {
        out.push_str("\n## slang compiler ceiling\n\n");
        out.push_str("| workload | wall ms | rss | exit | diagnostics |\n");
        out.push_str("| --- | ---: | ---: | ---: | ---: |\n");
        for sample in &report.slang {
            let rss = sample.rss_kb.map(|kb| format!("{kb} KB")).unwrap_or_else(|| "—".into());
            out.push_str(&format!(
                "| {} | {} | {rss} | {} | {} |\n",
                sample.workload, sample.wall_ms, sample.exit_code, sample.diagnostic_lines
            ));
        }
    }

    if !report.accuracy.is_empty() {
        out.push_str("\n## Accuracy vs slang-server\n\n");
        out.push_str(
            "| workload | server | probe | method | matched | extra | missing | nonempty |\n",
        );
        out.push_str("| --- | --- | --- | --- | ---: | ---: | ---: | --- |\n");
        for row in &report.accuracy {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} / {} |\n",
                row.workload,
                row.server,
                row.probe,
                short_method(&row.method),
                row.matched,
                row.extra,
                row.missing,
                yn(row.nonempty),
                yn(row.oracle_nonempty)
            ));
        }
    }

    if !report.notes.is_empty() {
        out.push_str("\n## Notes\n\n");
        for note in &report.notes {
            out.push_str(&format!("- {note}\n"));
        }
    }
    out
}

fn short_method(method: &str) -> &str {
    method.rsplit('/').next().unwrap_or(method)
}

fn yn(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn git_head(workspace_root: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(workspace_root)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".into())
}
