use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Catalog {
    pub workload: Vec<WorkloadSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkloadSpec {
    pub name: String,
    pub size: String,
    pub path: String,
    pub overlay: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct Workload {
    pub name: String,
    pub size: String,
    pub description: String,
    pub path: PathBuf,
    pub overlay: PathBuf,
    pub probes: Vec<Probe>,
    pub manifest: VideManifest,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VideManifest {
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub defines: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub top_modules: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeFile {
    pub probe: Vec<Probe>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Probe {
    pub id: String,
    pub file: String,
    /// 1-based editor line.
    pub line: u32,
    /// 1-based editor character.
    pub character: u32,
    pub methods: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub expect_label: Option<String>,
}

impl Workload {
    pub fn sources_present(&self) -> bool {
        self.path.is_dir()
            && fs::read_dir(&self.path).is_ok_and(|mut entries| entries.next().is_some())
    }

    pub fn probe_path(&self, probe: &Probe) -> PathBuf {
        self.path.join(&probe.file)
    }
}

impl Probe {
    pub fn lsp_line(&self) -> u32 {
        self.line.saturating_sub(1)
    }

    pub fn lsp_character(&self) -> u32 {
        self.character.saturating_sub(1)
    }
}

pub fn load_catalog(workspace_root: &Path) -> Result<Vec<Workload>> {
    let catalog_path = workspace_root.join("benches/workloads.toml");
    let text = fs::read_to_string(&catalog_path)
        .with_context(|| format!("failed to read {}", catalog_path.display()))?;
    let catalog: Catalog = toml::from_str(&text).context("invalid benches/workloads.toml")?;
    catalog.workload.into_iter().map(|spec| load_workload(workspace_root, spec)).collect()
}

fn load_workload(workspace_root: &Path, spec: WorkloadSpec) -> Result<Workload> {
    let path = workspace_root.join(spec.path);
    let overlay = workspace_root.join(spec.overlay);
    let manifest_path = overlay.join("vide.toml");
    let manifest_text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("missing overlay manifest {}", manifest_path.display()))?;
    let manifest: VideManifest = toml::from_str(&manifest_text)
        .with_context(|| format!("invalid {}", manifest_path.display()))?;
    let probes_path = overlay.join("probes.toml");
    let probes = if probes_path.exists() {
        let text = fs::read_to_string(&probes_path)?;
        toml::from_str::<ProbeFile>(&text)
            .with_context(|| format!("invalid {}", probes_path.display()))?
            .probe
    } else {
        Vec::new()
    };
    Ok(Workload {
        name: spec.name,
        size: spec.size,
        description: spec.description,
        path,
        overlay,
        probes,
        manifest,
    })
}

/// Copies tracked overlays into the submodule tree for the duration of a run.
pub struct OverlayGuard {
    created: Vec<PathBuf>,
}

impl OverlayGuard {
    pub fn apply(workload: &Workload) -> Result<Self> {
        let mut created = Vec::new();
        let vide_toml = workload.path.join("vide.toml");
        if vide_toml.exists() {
            bail!(
                "{} already has a vide.toml; refuse to overwrite. Track overlays only under {}",
                workload.path.display(),
                workload.overlay.display()
            );
        }
        fs::copy(workload.overlay.join("vide.toml"), &vide_toml)
            .with_context(|| format!("failed to install {}", vide_toml.display()))?;
        created.push(vide_toml);

        let slang_src = workload.overlay.join("slang-server.json");
        if slang_src.exists() {
            let slang_dir = workload.path.join(".slang");
            if !slang_dir.exists() {
                fs::create_dir_all(&slang_dir)?;
                created.push(slang_dir.clone());
            }
            let slang_dst = slang_dir.join("server.json");
            if slang_dst.exists() {
                bail!("{} already exists", slang_dst.display());
            }
            fs::copy(&slang_src, &slang_dst)?;
            created.push(slang_dst);
        }
        Ok(Self { created })
    }
}

impl Drop for OverlayGuard {
    fn drop(&mut self) {
        for path in self.created.iter().rev() {
            if path.is_dir() {
                let _ = fs::remove_dir(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
    }
}
