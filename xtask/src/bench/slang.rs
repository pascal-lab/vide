use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::workloads::Workload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlangSample {
    pub workload: String,
    pub wall_ms: u128,
    pub rss_kb: Option<u64>,
    pub exit_code: i32,
    pub diagnostic_lines: usize,
}

pub fn measure_slang_compile(workload: &Workload) -> Result<SlangSample> {
    let bin = resolve_slang()?;
    let files = collect_sources(workload)?;
    if files.is_empty() {
        bail!("no source files matched {}", workload.name);
    }
    let mut command = Command::new(&bin);
    command.arg("--error-limit=0");
    for dir in &workload.manifest.include_dirs {
        command.arg(format!("-I{}", dir));
    }
    for define in &workload.manifest.defines {
        command.arg(format!("-D{define}"));
    }
    for top in &workload.manifest.top_modules {
        command.arg("--top").arg(top);
    }
    command.args(&files);
    command.current_dir(&workload.path);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let start = Instant::now();
    let output = command.output().with_context(|| format!("failed to spawn {}", bin.display()))?;
    let wall_ms = start.elapsed().as_millis();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let diagnostic_lines =
        stderr.lines().chain(stdout.lines()).filter(|line| !line.is_empty()).count();

    Ok(SlangSample {
        workload: workload.name.clone(),
        wall_ms,
        rss_kb: None,
        exit_code: output.status.code().unwrap_or(-1),
        diagnostic_lines,
    })
}

fn resolve_slang() -> Result<PathBuf> {
    if let Ok(path) = env::var("SLANG_BIN") {
        return Ok(PathBuf::from(path));
    }
    for name in ["slang", "slang-driver"] {
        if let Some(path) = env::var_os("PATH").and_then(|path| {
            env::split_paths(&path).map(|dir| dir.join(name)).find(|candidate| candidate.is_file())
        }) {
            return Ok(path);
        }
    }
    bail!("slang not on PATH (set SLANG_BIN)");
}

fn collect_sources(workload: &Workload) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    visit(
        &workload.path,
        &workload.path,
        &workload.manifest.exclude,
        &workload.manifest.sources,
        &mut files,
    )?;
    files.sort();
    Ok(files)
}

fn visit(
    root: &Path,
    dir: &Path,
    exclude: &[String],
    sources: &[String],
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rel_str = rel.to_string_lossy();
        if exclude.iter().any(|pattern| glob_match(pattern, &rel_str)) {
            continue;
        }
        if path.is_dir() {
            visit(root, &path, exclude, sources, files)?;
            continue;
        }
        if !sources.is_empty() && !sources.iter().any(|pattern| glob_match(pattern, &rel_str)) {
            continue;
        }
        if let Some("sv" | "v" | "svh" | "vh") = path.extension().and_then(|ext| ext.to_str()) {
            files.push(rel.to_path_buf());
        }
    }
    Ok(())
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let trimmed = pattern.trim_end_matches("/**").trim_end_matches("**");
    path == pattern || path.starts_with(trimmed)
}
