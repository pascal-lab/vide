use std::{collections::BTreeSet, fs, io::ErrorKind};

use anyhow::{Context, bail};
use const_format::formatcp;
use utils::paths::AbsPathBuf;

pub const MANIFEST_FILE_NAME: &str = formatcp!("vide.toml");
pub const MANIFEST_FILE_NAMES: [&str; 1] = [MANIFEST_FILE_NAME];

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

        Ok(Self::UnconfiguredRoot(path.clone()))
    }

    pub fn toml_file_name(&self) -> Option<ProjectManifestFileName> {
        match self {
            ProjectManifest::Toml(path) => {
                path.file_name().and_then(ProjectManifestFileName::from_file_name)
            }
            ProjectManifest::UnconfiguredRoot(_) => None,
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
}
