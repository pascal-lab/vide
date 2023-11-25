use std::path::PathBuf;

use anyhow::Context;
use const_format::formatcp;
use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use smol_str::SmolStr;
use utils::paths::{sort_and_remove_subfolders, AbsPathBuf};

use crate::macro_def::{MacroAtom, MacroDef};

const IDENTIFIER_RE: &str = r"[a-zA-Z_][a-zA-Z0-9$_]*|\\\S* ";
static IDENT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})$")).unwrap());
static KV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})=(.*)$")).unwrap());

#[derive(Debug, Deserialize)]
struct TomlManifestSchema {
    #[serde(deserialize_with = "de_macros", default)]
    pub macros: MacroDef,
    #[serde(default)]
    pub included_files: Option<Vec<PathBuf>>,
    #[serde(default)]
    pub excluded_files: Vec<PathBuf>,
}

fn de_macros<'de, D>(deserializer: D) -> Result<MacroDef, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let res = Vec::<SmolStr>::deserialize(deserializer)?;
    let macros = res
        .into_iter()
        .map(|macr: SmolStr| {
            if IDENT_RE.is_match(&macr) {
                Ok(MacroAtom::Flag(macr))
            } else if let Some(caps) = KV_RE.captures(&macr) {
                let mut key: SmolStr = caps.get(1).unwrap().as_str().into();
                let value = caps.get(2).unwrap().as_str().into();
                if key.starts_with("\\") {
                    assert!(key.ends_with(" "));
                    key = key[1..key.len() - 1].into();
                }
                Ok(MacroAtom::KeyValue { key, value })
            } else {
                Err(serde::de::Error::custom(format!("Invalid macro definition: {macr}")))
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .collect::<FxHashSet<_>>();
    Ok(MacroDef { macros })
}

#[derive(Debug, PartialEq, Eq)]
pub struct TomlWorkspace {
    pub workspace_root: AbsPathBuf,
    pub macro_defs: MacroDef,
    pub included_files: Vec<AbsPathBuf>,
    pub excluded_files: Vec<AbsPathBuf>,
    pub package_files: Vec<AbsPathBuf>,
    pub is_lib: bool,
}

impl TomlWorkspace {
    pub fn load_from_file(toml: &AbsPathBuf, is_lib: bool) -> anyhow::Result<Self> {
        let toml_file =
            std::fs::read_to_string(toml).with_context(|| format!("failed to read {:?}", toml))?;

        let toml_schema: TomlManifestSchema =
            toml::from_str(&toml_file).with_context(|| format!("failed to parse {:?}", toml))?;

        let workspace_root = toml.parent().unwrap().to_path_buf();

        let mut excluded_files = toml_schema
            .excluded_files
            .into_iter()
            .map(|path| workspace_root.absolutize(path))
            .collect_vec();
        sort_and_remove_subfolders(&mut excluded_files);

        let mut included_files = Vec::new();
        let mut package_files = Vec::new();
        for path in toml_schema.included_files.unwrap_or_default().into_iter() {
            let path = workspace_root.absolutize(path);
            if excluded_files.iter().all(|excluded| !path.starts_with(excluded)) {
                if path.starts_with(&workspace_root) {
                    included_files.push(path);
                } else {
                    package_files.push(path);
                }
            }
        }
        sort_and_remove_subfolders(&mut included_files);

        if included_files.is_empty() {
            included_files.push(workspace_root.clone());
        }

        Ok(TomlWorkspace {
            workspace_root,
            macro_defs: toml_schema.macros,
            included_files,
            excluded_files,
            package_files,
            is_lib,
        })
    }

    pub fn default_from_path(path: &AbsPathBuf) -> Self {
        Self {
            workspace_root: path.clone(),
            macro_defs: MacroDef::default(),
            included_files: vec![path.clone()],
            excluded_files: vec![],
            package_files: vec![],
            is_lib: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_de_macros() {
        let toml = r#"

        "#;
        let toml_schema: TomlManifestSchema = toml::from_str(toml).unwrap();
        let mut macros = FxHashSet::default();
        macros.insert(MacroAtom::Flag("foo".into()));
        macros.insert(MacroAtom::Flag("bar".into()));
        macros.insert(MacroAtom::KeyValue { key: "FOO".into(), value: "bar".into() });
        macros.insert(MacroAtom::KeyValue { key: "BAR".into(), value: "foo".into() });
        macros.insert(MacroAtom::KeyValue { key: "BAZ".into(), value: "foo bar".into() });
        macros.insert(MacroAtom::KeyValue { key: "eqwe".into(), value: "123".into() });
        assert_eq!(toml_schema.macros, MacroDef { macros });
    }
}
