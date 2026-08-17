use std::{
    fs,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    client::LspClient,
    servers::ServerSpec,
    workloads::{Probe, ReadyProbe, Workload},
};

#[derive(Debug, Clone)]
pub struct MeasureConfig {
    pub warm_runs: usize,
}

impl Default for MeasureConfig {
    fn default() -> Self {
        Self { warm_runs: 10 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timing {
    pub millis: u128,
}

impl Timing {
    fn from_duration(duration: Duration) -> Self {
        Self { millis: duration.as_millis() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSample {
    pub probe: String,
    pub method: String,
    pub cold_ms: u128,
    pub warm_p50_ms: u128,
    pub warm_p95_ms: u128,
    pub after_edit_ms: u128,
    pub result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspSample {
    pub workload: String,
    pub size: String,
    pub server: String,
    pub oracle: bool,
    pub initialize_ms: u128,
    pub ready_ms: Option<u128>,
    pub rss_kb: Option<u64>,
    pub requests: Vec<RequestSample>,
    pub error: Option<String>,
}

pub fn measure_server(
    server: &ServerSpec,
    workload: &Workload,
    config: &MeasureConfig,
) -> Result<LspSample> {
    let mut client = LspClient::spawn(server, &workload.path)?;
    let start = Instant::now();
    client.initialize(&workload.path)?;
    let initialize_ms = start.elapsed().as_millis();

    let ready_ms = match &workload.ready {
        Some(ready) => Some(wait_until_ready(&mut client, workload, ready)?),
        None => None,
    };

    let mut opened = Vec::new();
    let mut versions = std::collections::HashMap::<std::path::PathBuf, i32>::new();
    let mut requests = Vec::new();
    for probe in &workload.probes {
        let path = workload.probe_path(probe);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read probe file {}", path.display()))?;
        if !opened.iter().any(|existing| existing == &path) {
            client.did_open(&path, &text)?;
            opened.push(path.clone());
            versions.insert(path.clone(), 1);
        }
        for method in &probe.methods {
            let lsp_method = lsp_method_name(method);
            match time_request(&mut client, probe, lsp_method, &path, &text, &mut versions, config)
            {
                Ok(sample) => requests.push(sample),
                Err(error) if is_unsupported_method(&error) => {
                    eprintln!("    skip {method}: not supported");
                }
                Err(error) => {
                    eprintln!("    {method} failed: {error:#}");
                    if client.child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                }
            }
        }
        if client.child.try_wait().ok().flatten().is_some() {
            break;
        }
    }

    let rss_kb = rss_kb(client.child.id());
    if client.child.try_wait().ok().flatten().is_none() {
        let _ = client.shutdown();
    }
    if requests.is_empty() {
        bail!("no successful requests");
    }
    Ok(LspSample {
        workload: workload.name.clone(),
        size: workload.size.clone(),
        server: server.id.to_owned(),
        oracle: server.is_oracle(),
        initialize_ms,
        ready_ms,
        rss_kb,
        requests,
        error: None,
    })
}

const READY_TIMEOUT: Duration = Duration::from_secs(60);

/// Blocks until the ready position resolves, so every `cold` below is the
/// latency of a real answer rather than of a server that is still indexing.
fn wait_until_ready(
    client: &mut LspClient,
    workload: &Workload,
    ready: &ReadyProbe,
) -> Result<u128> {
    let path = workload.ready_path(ready);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read ready probe file {}", path.display()))?;
    client.did_open(&path, &text)?;
    let start = Instant::now();
    while start.elapsed() < READY_TIMEOUT {
        let result = client.request_at(
            "textDocument/definition",
            &path,
            ready.lsp_line(),
            ready.lsp_character(),
        )?;
        if !is_empty_result(&result) {
            return Ok(start.elapsed().as_millis());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    bail!("{} never resolved the ready position at {}", workload.name, ready.file)
}

fn is_empty_result(result: &Value) -> bool {
    match result {
        Value::Null => true,
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

fn time_request(
    client: &mut LspClient,
    probe: &Probe,
    method: &str,
    path: &std::path::Path,
    text: &str,
    versions: &mut std::collections::HashMap<std::path::PathBuf, i32>,
    config: &MeasureConfig,
) -> Result<RequestSample> {
    let line = probe.lsp_line();
    let character = probe.lsp_character();
    let start = Instant::now();
    let result = client.request_at(method, path, line, character)?;
    let cold = start.elapsed();

    let mut warm = Vec::with_capacity(config.warm_runs);
    for _ in 0..config.warm_runs {
        let start = Instant::now();
        let _ = client.request_at(method, path, line, character)?;
        warm.push(start.elapsed());
    }
    warm.sort();
    let warm_p50 = percentile(&warm, 50);
    let warm_p95 = percentile(&warm, 95);

    let edited = format!("{text} // vide-bench-touch\n");
    let next = versions.get(path).copied().unwrap_or(1) + 1;
    client.did_change(path, next, &edited)?;
    let start = Instant::now();
    let _ = client.request_at(method, path, line, character)?;
    let after_edit = start.elapsed();
    client.did_change(path, next + 1, text)?;
    versions.insert(path.to_path_buf(), next + 1);

    Ok(RequestSample {
        probe: probe.id.clone(),
        method: method.to_owned(),
        cold_ms: Timing::from_duration(cold).millis,
        warm_p50_ms: Timing::from_duration(warm_p50).millis,
        warm_p95_ms: Timing::from_duration(warm_p95).millis,
        after_edit_ms: Timing::from_duration(after_edit).millis,
        result,
    })
}

fn percentile(sorted: &[Duration], pct: u32) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() - 1) * pct as usize) / 100;
    sorted[idx]
}

fn is_unsupported_method(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}").to_ascii_lowercase();
    text.contains("method not found") || text.contains("-32601")
}

fn lsp_method_name(method: &str) -> &'static str {
    match method {
        "definition" => "textDocument/definition",
        "hover" => "textDocument/hover",
        "references" => "textDocument/references",
        "completion" => "textDocument/completion",
        other => panic!("unknown probe method {other}"),
    }
}

fn rss_kb(pid: u32) -> Option<u64> {
    let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}
