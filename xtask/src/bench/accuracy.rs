use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::report::{AccuracyRow, BenchReport};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LocationKey {
    pub path: String,
    pub line: u32,
    pub character: u32,
}

pub fn score_accuracy(report: &mut BenchReport, workload: &str) {
    let samples: Vec<_> = report
        .lsp
        .iter()
        .filter(|sample| sample.workload == workload && sample.error.is_none())
        .cloned()
        .collect();
    let Some(oracle) = samples.iter().find(|sample| sample.oracle) else {
        report.notes.push(format!(
            "{workload}: no slang-server sample; accuracy scored pairwise against Vide only"
        ));
        return;
    };
    for sample in &samples {
        if sample.server == oracle.server {
            continue;
        }
        for request in &sample.requests {
            let Some(oracle_request) = oracle.requests.iter().find(|candidate| {
                candidate.probe == request.probe && candidate.method == request.method
            }) else {
                continue;
            };
            report.accuracy.push(compare_request(
                workload,
                &sample.server,
                request,
                oracle_request,
            ));
        }
    }
}

fn compare_request(
    workload: &str,
    server: &str,
    got: &super::measure::RequestSample,
    oracle: &super::measure::RequestSample,
) -> AccuracyRow {
    match got.method.as_str() {
        "textDocument/definition" | "textDocument/references" => {
            let got_locs = locations(&got.result);
            let oracle_locs = locations(&oracle.result);
            let matched = got_locs.iter().filter(|loc| oracle_locs.contains(loc)).count();
            let extra = got_locs.len().saturating_sub(matched);
            let missing = oracle_locs.len().saturating_sub(matched);
            AccuracyRow {
                workload: workload.to_owned(),
                server: server.to_owned(),
                probe: got.probe.clone(),
                method: got.method.clone(),
                kind: "locations".to_owned(),
                matched,
                extra,
                missing,
                oracle_count: oracle_locs.len(),
                got_count: got_locs.len(),
                nonempty: !got_locs.is_empty(),
                oracle_nonempty: !oracle_locs.is_empty(),
            }
        }
        "textDocument/hover" => {
            let got_hit = hover_nonempty(&got.result);
            let oracle_hit = hover_nonempty(&oracle.result);
            AccuracyRow {
                workload: workload.to_owned(),
                server: server.to_owned(),
                probe: got.probe.clone(),
                method: got.method.clone(),
                kind: "hover".to_owned(),
                matched: usize::from(got_hit == oracle_hit && got_hit),
                extra: usize::from(got_hit && !oracle_hit),
                missing: usize::from(!got_hit && oracle_hit),
                oracle_count: usize::from(oracle_hit),
                got_count: usize::from(got_hit),
                nonempty: got_hit,
                oracle_nonempty: oracle_hit,
            }
        }
        "textDocument/completion" => {
            let got_hit = completion_nonempty(&got.result);
            let oracle_hit = completion_nonempty(&oracle.result);
            AccuracyRow {
                workload: workload.to_owned(),
                server: server.to_owned(),
                probe: got.probe.clone(),
                method: got.method.clone(),
                kind: "completion".to_owned(),
                matched: usize::from(got_hit && oracle_hit),
                extra: 0,
                missing: usize::from(!got_hit && oracle_hit),
                oracle_count: usize::from(oracle_hit),
                got_count: usize::from(got_hit),
                nonempty: got_hit,
                oracle_nonempty: oracle_hit,
            }
        }
        other => AccuracyRow {
            workload: workload.to_owned(),
            server: server.to_owned(),
            probe: got.probe.clone(),
            method: other.to_owned(),
            kind: "unknown".to_owned(),
            matched: 0,
            extra: 0,
            missing: 0,
            oracle_count: 0,
            got_count: 0,
            nonempty: false,
            oracle_nonempty: false,
        },
    }
}

fn locations(value: &Value) -> Vec<LocationKey> {
    let mut out = Vec::new();
    collect_locations(value, &mut out);
    out.sort_by(|a, b| (&a.path, a.line, a.character).cmp(&(&b.path, b.line, b.character)));
    out.dedup();
    out
}

fn collect_locations(value: &Value, out: &mut Vec<LocationKey>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_locations(item, out);
            }
        }
        Value::Object(map) => {
            if let Some(target) = map.get("targetUri").or_else(|| map.get("uri"))
                && let Some(uri) = target.as_str()
            {
                let range = map
                    .get("targetRange")
                    .or_else(|| map.get("targetSelectionRange"))
                    .or_else(|| map.get("range"));
                if let Some((line, character)) = range_start(range) {
                    out.push(LocationKey { path: uri_to_rel(uri), line, character });
                    return;
                }
            }
            if let Some(loc) = map.get("location") {
                collect_locations(loc, out);
            }
        }
        _ => {}
    }
}

fn range_start(range: Option<&Value>) -> Option<(u32, u32)> {
    let start = range?.get("start")?;
    Some((start.get("line")?.as_u64()? as u32, start.get("character")?.as_u64()? as u32))
}

fn uri_to_rel(uri: &str) -> String {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or(path).to_owned()
}

fn hover_nonempty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Object(map) => map.get("contents").is_some_and(|contents| !contents.is_null()),
        _ => true,
    }
}

fn completion_nonempty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => {
            map.get("items").and_then(Value::as_array).is_some_and(|items| !items.is_empty())
        }
        _ => true,
    }
}
