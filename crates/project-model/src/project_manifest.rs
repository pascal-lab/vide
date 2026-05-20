use std::{collections::BTreeSet, fs, io::ErrorKind};

use anyhow::{Context, bail};
use const_format::formatcp;
use itertools::Itertools;
use utils::paths::AbsPathBuf;

pub const MANIFEST_FILE_NAME: &str = formatcp!("vizsla.toml");
pub const LEGACY_MANIFEST_FILE_NAME: &str = formatcp!("vizsla_config.toml");
pub const MANIFEST_FILE_NAMES: [&str; 2] = [MANIFEST_FILE_NAME, LEGACY_MANIFEST_FILE_NAME];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ProjectManifest {
    Toml(AbsPathBuf),
    UnconfiguredRoot(AbsPathBuf),
}

pub fn is_manifest_file_name(file_name: &str) -> bool {
    MANIFEST_FILE_NAMES.contains(&file_name)
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

        (manifests.into_iter().collect_vec(), errors)
    }

    pub fn from_path(path: &AbsPathBuf) -> anyhow::Result<ProjectManifest> {
        if is_manifest_file_name(path.file_name().unwrap_or_default()) {
            return Self::from_toml(path);
        }

        let metadata =
            fs::metadata(path).with_context(|| format!("project path does not exist: {path}"))?;
        if !metadata.is_dir() {
            bail!(
                "project path must be a directory or one of {}: {path}",
                MANIFEST_FILE_NAMES.iter().join(", ")
            );
        }

        for manifest_file_name in MANIFEST_FILE_NAMES {
            let manifest = path.join(manifest_file_name);
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

    fn from_toml(path: &AbsPathBuf) -> anyhow::Result<Self> {
        if path.parent().is_none() {
            bail!("bad manifest path: {path}");
        }

        if !is_manifest_file_name(path.file_name().unwrap_or_default()) {
            bail!(
                "manifest path must point to one of {}: {path}",
                MANIFEST_FILE_NAMES.iter().join(", ")
            );
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

    use super::{LEGACY_MANIFEST_FILE_NAME, MANIFEST_FILE_NAME, ProjectManifest};

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
    fn from_path_uses_legacy_workspace_root_manifest() {
        let root = TestDir::new("manifest-legacy-root");
        let manifest_path = root.join(LEGACY_MANIFEST_FILE_NAME);
        fs::write(&manifest_path, r#"top_modules = ["legacy"]"#).unwrap();

        let root = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root).unwrap();

        assert_eq!(manifest, ProjectManifest::Toml(manifest_path));
    }

    #[test]
    fn from_path_prefers_workspace_root_manifest_over_legacy_manifest() {
        let root = TestDir::new("manifest-preferred-root");
        let manifest_path = root.join(MANIFEST_FILE_NAME);
        let legacy_manifest_path = root.join(LEGACY_MANIFEST_FILE_NAME);
        fs::write(&manifest_path, r#"top_modules = ["root"]"#).unwrap();
        fs::write(&legacy_manifest_path, r#"top_modules = ["legacy"]"#).unwrap();

        let root = root.path().to_path_buf();
        let manifest = ProjectManifest::from_path(&root).unwrap();

        assert_eq!(manifest, ProjectManifest::Toml(manifest_path));
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
