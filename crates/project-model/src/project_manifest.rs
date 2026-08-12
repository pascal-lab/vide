use std::{collections::BTreeSet, fs, io::ErrorKind};

use anyhow::{Context, bail};
use const_format::formatcp;
use utils::paths::AbsPathBuf;

pub const MANIFEST_FILE_NAME: &str = formatcp!("vide.toml");
pub const MANIFEST_FILE_NAMES: [&str; 1] = [MANIFEST_FILE_NAME];
pub const FUSESOC_CORE_EXTENSIONS: [&str; 1] = ["core"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ProjectManifestFileName {
    Primary,
}

impl ProjectManifestFileName {
    pub const DISCOVERY_ORDER: [ProjectManifestFileName; 1] = [ProjectManifestFileName::Primary];

    pub const fn as_str(self) -> &'static str {
        match self {
            ProjectManifestFileName::Primary => MANIFEST_FILE_NAME,
        }
    }

    pub fn from_file_name(file_name: &str) -> Option<Self> {
        match file_name {
            MANIFEST_FILE_NAME => Some(ProjectManifestFileName::Primary),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ProjectManifest {
    Toml(AbsPathBuf),
    /// A FuseSoC CAPI2 `.core` file found in the workspace root.
    FuseSocCore(AbsPathBuf),
    UnconfiguredRoot(AbsPathBuf),
}

pub fn is_manifest_file_name(file_name: &str) -> bool {
    ProjectManifestFileName::from_file_name(file_name).is_some()
}

impl ProjectManifest {
    pub fn from_paths(paths: &[AbsPathBuf]) -> (Vec<ProjectManifest>, Vec<anyhow::Error>) {
        let mut manifests = BTreeSet::new();
        let mut errors = Vec::new();

        for path in paths {
            match Self::from_path(path) {
                Ok(manifest) => {
                    manifests.insert(manifest);
                }
                Err(error) => errors.push(error),
            }
        }

        (manifests.into_iter().collect(), errors)
    }

    pub fn from_path(path: &AbsPathBuf) -> anyhow::Result<ProjectManifest> {
        if is_manifest_file_name(path.file_name().unwrap_or_default()) {
            return Self::from_toml(path);
        }
        if path.extension().is_some_and(|ext| ext == "core") {
            return Self::from_fusesoc_core(path);
        }

        let metadata =
            fs::metadata(path).with_context(|| format!("project path does not exist: {path}"))?;
        if !metadata.is_dir() {
            bail!("project path must be a directory or {MANIFEST_FILE_NAME}: {path}");
        }

        for manifest_file_name in ProjectManifestFileName::DISCOVERY_ORDER {
            let manifest = path.join(manifest_file_name.as_str());
            match fs::metadata(&manifest) {
                Ok(metadata) if metadata.is_file() => return Self::from_toml(&manifest),
                Ok(_) => bail!("project manifest path is not a file: {manifest}"),
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(err).with_context(|| format!("failed to inspect {manifest}"));
                }
            }
        }

        // No vide.toml — look for a single .core file in the workspace root.
        if let Some(core_path) = find_single_core_file(path) {
            return Self::from_fusesoc_core(&core_path);
        }

        Ok(Self::UnconfiguredRoot(path.clone()))
    }

    pub fn toml_file_name(&self) -> Option<ProjectManifestFileName> {
        match self {
            ProjectManifest::Toml(path) => {
                path.file_name().and_then(ProjectManifestFileName::from_file_name)
            }
            ProjectManifest::FuseSocCore(_) | ProjectManifest::UnconfiguredRoot(_) => None,
        }
    }

    fn from_toml(path: &AbsPathBuf) -> anyhow::Result<Self> {
        if path.parent().is_none() {
            bail!("bad manifest path: {path}");
        }

        if ProjectManifestFileName::from_file_name(path.file_name().unwrap_or_default()).is_none() {
            bail!("manifest path must point to {MANIFEST_FILE_NAME}: {path}");
        }

        let metadata = fs::metadata(path)
            .with_context(|| format!("project manifest path does not exist: {path}"))?;
        if !metadata.is_file() {
            bail!("project manifest path is not a file: {path}");
        }

        Ok(ProjectManifest::Toml(path.clone()))
    }

    fn from_fusesoc_core(path: &AbsPathBuf) -> anyhow::Result<Self> {
        if path.parent().is_none() {
            bail!("bad .core path: {path}");
        }

        let metadata = fs::metadata(path)
            .with_context(|| format!("project .core path does not exist: {path}"))?;
        if !metadata.is_file() {
 bail!("project .core path is not a file: {path}");
        }

        Ok(ProjectManifest::FuseSocCore(path.clone()))
    }
}

/// Find a single `.core` file directly in `dir`.  Returns `None` if there
/// are zero or multiple `.core` files (ambiguous).
fn find_single_core_file(dir: &AbsPathBuf) -> Option<AbsPathBuf> {
    let entries = fs::read_dir(dir.as_path()).ok()?;
    let mut core_files: Vec<AbsPathBuf> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "core") {
            if let Some(abs) = utils::paths::abs_path_buf_from_path_buf(path) {
                core_files.push(abs);
            }
        }
    }
    match core_files.len() {
        1 => Some(core_files.into_iter().next().unwrap()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use utils::test_support::TestDir;

    use super::{MANIFEST_FILE_NAME, ProjectManifest, ProjectManifestFileName};

    #[test]
    fn from_path_does_not_use_parent_manifest() {
        let base = TestDir::new("manifest-parent");
        let child = base.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(base.join(MANIFEST_FILE_NAME), r#"top_modules = ["parent"]"#).unwrap();

        let child_abs = child;
        let manifest = ProjectManifest::from_path(&child_abs).unwrap();

        assert_eq!(manifest, ProjectManifest::UnconfiguredRoot(child_abs));
    }

    #[test]
    fn from_path_uses_workspace_root_manifest() {
        let root = TestDir::new("manifest-root");
        let manifest_path = root.join(MANIFEST_FILE_NAME);
        fs::write(&manifest_path, r#"top_modules = ["root"]"#).unwrap();

        let root = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root).unwrap();

        assert_eq!(manifest, ProjectManifest::Toml(manifest_path));
    }

    #[test]
    fn classifies_manifest_file_names() {
        assert_eq!(
            ProjectManifestFileName::from_file_name(MANIFEST_FILE_NAME),
            Some(ProjectManifestFileName::Primary)
        );
        assert_eq!(ProjectManifestFileName::from_file_name("vizsla.toml"), None);
    }

    #[test]
    fn from_path_does_not_use_child_manifest() {
        let root = TestDir::new("manifest-child");
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join(MANIFEST_FILE_NAME), r#"top_modules = ["child"]"#).unwrap();

        let root_abs = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root_abs).unwrap();

        assert_eq!(manifest, ProjectManifest::UnconfiguredRoot(root_abs));
    }

    #[test]
    fn from_path_rejects_non_manifest_file() {
        let root = TestDir::new("manifest-file");
        let file = root.join("top.sv");
        fs::write(&file, "module top; endmodule\n").unwrap();

        let error = ProjectManifest::from_path(&file).unwrap_err();

        assert!(error.to_string().contains("must be a directory"));
    }

    #[test]
    fn from_path_discovers_single_core_file() {
        let root = TestDir::new("fusesoc-single-core");
        let core_path = root.join("top.core");
        fs::write(&core_path, "CAPI=2:\nname: v:l:top:1.0\n").unwrap();

        let root_abs = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root_abs).unwrap();

        assert_eq!(manifest, ProjectManifest::FuseSocCore(core_path));
    }

    #[test]
    fn from_path_rejects_ambiguous_multiple_core_files() {
        let root = TestDir::new("fusesoc-ambiguous-cores");
        fs::write(root.join("a.core"), "CAPI=2:\nname: v:l:a:1.0\n").unwrap();
        fs::write(root.join("b.core"), "CAPI=2:\nname: v:l:b:1.0\n").unwrap();

        let root_abs = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root_abs).unwrap();

        // Multiple cores is ambiguous — falls back to unconfigured root.
        assert_eq!(manifest, ProjectManifest::UnconfiguredRoot(root_abs));
    }

    #[test]
    fn from_path_prefers_vide_toml_over_core() {
        let root = TestDir::new("fusesoc-and-toml");
        fs::write(root.join("top.core"), "CAPI=2:\nname: v:l:top:1.0\n").unwrap();
        let toml_path = root.join(MANIFEST_FILE_NAME);
        fs::write(&toml_path, r#"top_modules = ["top"]"#).unwrap();

        let root_abs = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root_abs).unwrap();

        assert_eq!(manifest, ProjectManifest::Toml(toml_path));
    }

    #[test]
    fn from_path_accepts_core_file_directly() {
        let root = TestDir::new("fusesoc-direct");
        let core_path = root.join("top.core");
        fs::write(&core_path, "CAPI=2:\nname: v:l:top:1.0\n").unwrap();

        let manifest = ProjectManifest::from_path(&core_path).unwrap();
        assert_eq!(manifest, ProjectManifest::FuseSocCore(core_path));
    }
}
