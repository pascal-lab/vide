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
    pub fn discover_all(paths: &[AbsPathBuf]) -> Vec<ProjectManifest> {
        paths
            .iter()
            .filter_map(|path| ProjectManifest::discover(path).ok())
            .flatten()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect_vec()
    }

    pub fn discover(path: &AbsPathBuf) -> anyhow::Result<Vec<ProjectManifest>> {
        if let Ok(manifest) = Self::from_toml(path) {
            return Ok(vec![manifest]);
        }

        // find in parent dirs
        let mut cur = Some(path.as_path());
        while let Some(path) = cur {
            let candidate = path.join(MANIFEST_FILE_NAME);

            if fs::metadata(&candidate).is_ok()
                && let Ok(manifest) = Self::from_toml(&candidate)
            {
                return Ok(vec![manifest]);
            }

            cur = path.parent();
        }

        // Only one level down to avoid cycles
        let entities = fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|it| it.path().join(MANIFEST_FILE_NAME))
            .filter(|it| it.exists())
            .filter_map(|it| AbsPathBuf::try_from(it).ok())
            .filter_map(|it| Self::from_toml(&it).ok())
            .collect_vec();

        if entities.is_empty() {
            return Ok(vec![Self::Discover(path.clone())]);
        }

        Ok(entities)
    }

    fn from_toml(path: &AbsPathBuf) -> Result<Self, String> {
        if path.parent().is_none() {
            return Err(String::from("Bad manifest path: {path}"));
        }

        if path.file_name().unwrap_or_default() != MANIFEST_FILE_NAME {
            return Err(String::from("Project root must point to {MANIFEST_FILE_NAME}: {path}"));
        }

        Ok(ProjectManifest::Toml(path.clone()))
    }
}
