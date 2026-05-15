use std::sync::LazyLock;

use anyhow::Context;
use const_format::formatcp;
use itertools::Itertools;
use regex::Regex;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use smol_str::SmolStr;
use utils::paths::{AbsPathBuf, Utf8PathBuf, sort_and_remove_subfolders};

use crate::macro_def::{MacroAtom, MacroDef};

const DEFAULT_TOP_MODULE: &str = "main";
const IDENTIFIER_RE: &str = r"[a-zA-Z_][a-zA-Z0-9$_]*|\\\S* ";
static IDENT_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})$")));
static KV_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})=(.*)$")));

#[derive(Debug, Deserialize)]
struct TomlManifestSchema {
    #[serde(default = "default_top_module")]
    pub top_module: String,
    #[serde(deserialize_with = "de_macros", default)]
    pub macros: MacroDef,
    #[serde(default)]
    pub include: Option<Vec<Utf8PathBuf>>,
    #[serde(default)]
    pub exclude: Vec<Utf8PathBuf>,
}

fn default_top_module() -> String {
    DEFAULT_TOP_MODULE.to_owned()
}

fn de_macros<'de, D>(deserializer: D) -> Result<MacroDef, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let res = Vec::<SmolStr>::deserialize(deserializer)?;
    let ident_re = IDENT_RE.as_ref().map_err(|err| {
        serde::de::Error::custom(format!("invalid macro identifier regex: {err}"))
    })?;
    let kv_re = KV_RE
        .as_ref()
        .map_err(|err| serde::de::Error::custom(format!("invalid macro key-value regex: {err}")))?;
    let macros = res
        .into_iter()
        .map(|macr: SmolStr| {
            if ident_re.is_match(&macr) {
                Ok(MacroAtom::Flag(macr))
            } else if let Some(caps) = kv_re.captures(&macr) {
                let Some(key_match) = caps.get(1) else {
                    return Err(serde::de::Error::custom(format!(
                        "Invalid macro definition: {macr}"
                    )));
                };
                let Some(value_match) = caps.get(2) else {
                    return Err(serde::de::Error::custom(format!(
                        "Invalid macro definition: {macr}"
                    )));
                };
                let mut key: SmolStr = key_match.as_str().into();
                let value = value_match.as_str().into();
                if key.starts_with('\\') {
                    let Some(stripped) =
                        key.strip_prefix('\\').and_then(|key| key.strip_suffix(' '))
                    else {
                        return Err(serde::de::Error::custom(format!(
                            "Invalid escaped macro name: {macr}"
                        )));
                    };
                    key = stripped.into();
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
    pub top_module: String,
    pub workspace_root: AbsPathBuf,
    pub macro_defs: MacroDef,
    pub include: Vec<AbsPathBuf>,
    pub exclude: Vec<AbsPathBuf>,
    pub package: Vec<AbsPathBuf>,
    pub is_lib: bool,
}

impl TomlWorkspace {
    pub fn load_from_file(toml: &AbsPathBuf, is_lib: bool) -> anyhow::Result<Self> {
        let toml_file =
            std::fs::read_to_string(toml).with_context(|| format!("failed to read {:?}", toml))?;

        let toml_schema: TomlManifestSchema =
            toml::from_str(&toml_file).with_context(|| format!("failed to parse {:?}", toml))?;

        let top_module = toml_schema.top_module;
        let workspace_root = toml
            .parent()
            .with_context(|| format!("manifest path has no parent: {toml}"))?
            .to_path_buf();

        let mut exclude = toml_schema
            .exclude
            .into_iter()
            .map(|path| workspace_root.absolutize(path))
            .collect_vec();
        sort_and_remove_subfolders(&mut exclude);

        let mut include = Vec::new();
        let mut package = Vec::new();
        for path in toml_schema.include.unwrap_or_default().into_iter() {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                if path.starts_with(&workspace_root) {
                    include.push(path);
                } else {
                    package.push(path);
                }
            }
        }
        sort_and_remove_subfolders(&mut include);

        if include.is_empty() {
            include.push(workspace_root.clone());
        }

        Ok(TomlWorkspace {
            top_module,
            workspace_root,
            macro_defs: toml_schema.macros,
            include,
            exclude,
            package,
            is_lib,
        })
    }

    pub fn default_from_path(path: &AbsPathBuf) -> Self {
        Self {
            top_module: String::from(DEFAULT_TOP_MODULE),
            workspace_root: path.clone(),
            macro_defs: MacroDef::default(),
            include: vec![path.clone()],
            exclude: vec![],
            package: vec![],
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
top_module = "main"
macros = [
    "foo",
    "bar",
    "FOO=bar",
    "BAR=foo",
    "BAZ=foo bar",
    "eqwe=123",
]
        "#;
        let toml_schema: TomlManifestSchema = toml::from_str(toml).unwrap();
        assert_eq!(toml_schema.top_module, DEFAULT_TOP_MODULE);
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
