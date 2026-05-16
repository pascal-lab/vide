use std::{collections::BTreeSet, fs, io::ErrorKind};

use anyhow::{Context, bail};
use const_format::formatcp;
use itertools::Itertools;
use utils::paths::AbsPathBuf;

pub const MANIFEST_FILE_NAME: &str = formatcp!("vizsla_config.toml");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ProjectManifest {
    Toml(AbsPathBuf),
    UnconfiguredRoot(AbsPathBuf),
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
        if path.file_name().unwrap_or_default() == MANIFEST_FILE_NAME {
            return Self::from_toml(path);
        }

        let metadata =
            fs::metadata(path).with_context(|| format!("project path does not exist: {path}"))?;
        if !metadata.is_dir() {
            bail!("project path must be a directory or {MANIFEST_FILE_NAME}: {path}");
        }

        let manifest = path.join(MANIFEST_FILE_NAME);
        match fs::metadata(&manifest) {
            Ok(metadata) if metadata.is_file() => Self::from_toml(&manifest),
            Ok(_) => bail!("project manifest path is not a file: {manifest}"),
            Err(err) if err.kind() == ErrorKind::NotFound => {
                Ok(Self::UnconfiguredRoot(path.clone()))
            }
            Err(err) => Err(err).with_context(|| format!("failed to inspect {manifest}")),
        }
    }

    fn from_toml(path: &AbsPathBuf) -> anyhow::Result<Self> {
        if path.parent().is_none() {
            bail!("bad manifest path: {path}");
        }

        if path.file_name().unwrap_or_default() != MANIFEST_FILE_NAME {
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
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use utils::paths::AbsPathBuf;

    use super::{MANIFEST_FILE_NAME, ProjectManifest};

    fn temp_project_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("vizsla-manifest-{name}-{}-{stamp}", std::process::id()))
    }

    #[test]
    fn from_path_does_not_use_parent_manifest() {
        let base = temp_project_dir("parent");
        let child = base.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(base.join(MANIFEST_FILE_NAME), r#"top_modules = ["parent"]"#).unwrap();

        let child_abs = AbsPathBuf::try_from(child.clone()).unwrap();
        let manifest = ProjectManifest::from_path(&child_abs).unwrap();

        let _ = fs::remove_dir_all(base);

        assert_eq!(manifest, ProjectManifest::UnconfiguredRoot(child_abs));
    }

    #[test]
    fn from_path_uses_workspace_root_manifest() {
        let root = temp_project_dir("root");
        fs::create_dir_all(&root).unwrap();
        let manifest_path = root.join(MANIFEST_FILE_NAME);
        fs::write(&manifest_path, r#"top_modules = ["root"]"#).unwrap();

        let manifest =
            ProjectManifest::from_path(&AbsPathBuf::try_from(root.clone()).unwrap()).unwrap();

        let _ = fs::remove_dir_all(root);

        assert_eq!(manifest, ProjectManifest::Toml(AbsPathBuf::try_from(manifest_path).unwrap()));
    }

    #[test]
    fn from_path_does_not_use_child_manifest() {
        let root = temp_project_dir("child");
        let child = root.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::write(child.join(MANIFEST_FILE_NAME), r#"top_modules = ["child"]"#).unwrap();

        let root_abs = AbsPathBuf::try_from(root.clone()).unwrap();
        let manifest = ProjectManifest::from_path(&root_abs).unwrap();

        let _ = fs::remove_dir_all(root);

        assert_eq!(manifest, ProjectManifest::UnconfiguredRoot(root_abs));
    }

    #[test]
    fn from_path_rejects_non_manifest_file() {
        let root = temp_project_dir("file");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("top.sv");
        fs::write(&file, "module top; endmodule\n").unwrap();

        let error = ProjectManifest::from_path(&AbsPathBuf::try_from(file).unwrap()).unwrap_err();

        let _ = fs::remove_dir_all(root);

        assert!(error.to_string().contains("must be a directory"));
    }
}
