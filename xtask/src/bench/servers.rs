use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerKind {
    Vide,
    SlangServer,
    Verible,
    Svls,
}

impl ServerKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Vide => "vide",
            Self::SlangServer => "slang-server",
            Self::Verible => "verible",
            Self::Svls => "svls",
        }
    }

    fn from_id(id: &str) -> Result<Self> {
        match id {
            "vide" => Ok(Self::Vide),
            "slang-server" => Ok(Self::SlangServer),
            "verible" | "verible-verilog-ls" => Ok(Self::Verible),
            "svls" => Ok(Self::Svls),
            other => bail!("unknown server {other}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerSpec {
    pub kind: ServerKind,
    pub id: &'static str,
    pub bin: PathBuf,
}

impl ServerSpec {
    pub fn is_oracle(&self) -> bool {
        self.kind == ServerKind::SlangServer
    }
}

pub fn discover_servers(workspace_root: &Path, filter: &[String]) -> Result<Vec<ServerSpec>> {
    let wanted: Option<Vec<ServerKind>> = if filter.is_empty() {
        None
    } else {
        Some(filter.iter().map(|id| ServerKind::from_id(id)).collect::<Result<Vec<_>>>()?)
    };
    let mut servers = Vec::new();
    for kind in [ServerKind::Vide, ServerKind::SlangServer, ServerKind::Verible, ServerKind::Svls] {
        if wanted.as_ref().is_some_and(|set| !set.contains(&kind)) {
            continue;
        }
        match resolve_bin(workspace_root, kind) {
            Ok(bin) => servers.push(ServerSpec { kind, id: kind.id(), bin }),
            Err(error) if kind == ServerKind::Vide => return Err(error),
            Err(error) => eprintln!("skip {}: {error:#}", kind.id()),
        }
    }
    Ok(servers)
}

fn resolve_bin(workspace_root: &Path, kind: ServerKind) -> Result<PathBuf> {
    match kind {
        ServerKind::Vide => resolve_vide(workspace_root),
        ServerKind::SlangServer => resolve_on_path("SLANG_SERVER_BIN", "slang-server"),
        ServerKind::Verible => resolve_on_path("VERIBLE_LS_BIN", "verible-verilog-ls"),
        ServerKind::Svls => resolve_on_path("SVLS_BIN", "svls"),
    }
}

fn resolve_vide(workspace_root: &Path) -> Result<PathBuf> {
    if let Ok(path) = env::var("VIDE_BIN") {
        return Ok(PathBuf::from(path));
    }
    let release = workspace_root.join("target/release/vide");
    if !release.exists() {
        let status = Command::new("cargo")
            .args(["build", "--release", "-p", "vide"])
            .current_dir(workspace_root)
            .status()
            .context("failed to spawn cargo build -p vide")?;
        if !status.success() {
            bail!("cargo build --release -p vide failed");
        }
    }
    if !release.exists() {
        bail!("Vide binary missing at {}", release.display());
    }
    Ok(release)
}

fn resolve_on_path(env_key: &str, name: &str) -> Result<PathBuf> {
    if let Ok(path) = env::var(env_key) {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(path);
        }
        bail!("{env_key} points at missing {}", path.display());
    }
    which(name).with_context(|| format!("{name} not on PATH (set {env_key} to override)"))
}

fn which(name: &str) -> Result<PathBuf> {
    let path = env::var_os("PATH").context("PATH is unset")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Ok(exe);
            }
        }
    }
    let _ = fs::metadata(name);
    bail!("{name} not found");
}
