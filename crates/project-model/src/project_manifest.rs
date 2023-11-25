use std::{collections::BTreeSet, fs};

use const_format::formatcp;
use itertools::Itertools;
use utils::paths::AbsPathBuf;

pub const MANIFEST_FILE_NAME: &str = formatcp!("vizsla_config.toml");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum ProjectManifest {
    Toml(AbsPathBuf),
    Discover(AbsPathBuf),
}

impl ProjectManifest {
    fn validate_toml(path: &AbsPathBuf) -> Result<(), String> {
        if path.parent() == None {
            return Err(String::from("Bad manifest path: {path}"));
        }

        if path.file_name().unwrap_or_default() != MANIFEST_FILE_NAME {
            return Err(String::from("Project root must point to {MANIFEST_FILE_NAME}: {path}"));
        }

        Ok(())
    }

    fn from_toml(path: AbsPathBuf) -> Result<Self, String> {
        Self::validate_toml(&path)?;
        Ok(ProjectManifest::Toml(path))
    }

    fn from_toml_ref(path: &AbsPathBuf) -> Result<Self, String> {
        Self::validate_toml(path)?;
        Ok(ProjectManifest::Toml(path.clone()))
    }

    pub fn discover(path: &AbsPathBuf) -> anyhow::Result<Vec<ProjectManifest>> {
        if let Ok(manifest) = Self::from_toml_ref(path) {
            return Ok(vec![manifest]);
        }

        // find in parent dirs
        let mut cur = Some(path.as_path());
        while let Some(path) = cur {
            let candidate = path.join(MANIFEST_FILE_NAME);

            if fs::metadata(&candidate).is_ok() {
                if let Ok(manifest) = Self::from_toml(candidate) {
                    return Ok(vec![manifest]);
                }
            }

            cur = path.parent();
        }

        // Only one level down to avoid cycles
        let entities = fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|it| it.path().join(MANIFEST_FILE_NAME))
            .filter(|x| x.exists())
            .filter_map(|x| Self::from_toml(AbsPathBuf::assert(x)).ok())
            .collect_vec();

        if entities.is_empty() {
            return Ok(vec![Self::Discover(path.clone())]);
        }

        Ok(entities)
    }

    pub fn discover_all(paths: &[AbsPathBuf]) -> Vec<ProjectManifest> {
        paths
            .iter()
            .filter_map(|path| ProjectManifest::discover(path).ok())
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect_vec()
    }
}
