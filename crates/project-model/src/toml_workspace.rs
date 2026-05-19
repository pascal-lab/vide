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

const IDENTIFIER_RE: &str = r"[a-zA-Z_][a-zA-Z0-9$_]*|\\\S* ";
static IDENT_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})$")));
static KV_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})=(.*)$")));

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TomlManifestSchema {
    #[serde(default)]
    pub top_modules: Vec<String>,
    #[serde(deserialize_with = "de_macros", default)]
    pub defines: MacroDef,
    #[serde(default)]
    pub sources: Option<Vec<Utf8PathBuf>>,
    #[serde(default)]
    pub include_dirs: Option<Vec<Utf8PathBuf>>,
    #[serde(default)]
    pub libraries: Vec<Utf8PathBuf>,
    #[serde(default)]
    pub exclude: Vec<Utf8PathBuf>,
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
    pub top_modules: Vec<String>,
    pub workspace_root: AbsPathBuf,
    pub macro_defs: MacroDef,
    pub sources: Vec<AbsPathBuf>,
    pub include_dirs: Vec<AbsPathBuf>,
    pub exclude: Vec<AbsPathBuf>,
    pub package: Vec<AbsPathBuf>,
    pub is_lib: bool,
    pub has_manifest: bool,
}

impl TomlWorkspace {
    pub fn load_from_file(toml: &AbsPathBuf, is_lib: bool) -> anyhow::Result<Self> {
        let toml_file =
            std::fs::read_to_string(toml).with_context(|| format!("failed to read {:?}", toml))?;

        let toml_schema: TomlManifestSchema =
            toml::from_str(&toml_file).with_context(|| format!("failed to parse {:?}", toml))?;

        let top_modules = toml_schema.top_modules;
        let workspace_root = toml
            .parent()
            .with_context(|| format!("manifest path has no parent: {toml}"))?
            .to_path_buf();
        let macro_defs = toml_schema.defines;

        let mut exclude = toml_schema
            .exclude
            .into_iter()
            .map(|path| workspace_root.absolutize(path))
            .collect_vec();
        sort_and_remove_subfolders(&mut exclude);

        let sources_was_configured = toml_schema.sources.is_some();
        let include_dirs_was_configured = toml_schema.include_dirs.is_some();
        let mut sources = Vec::new();
        let mut include_dirs = Vec::new();
        let mut package = Vec::new();

        for path in toml_schema.sources.unwrap_or_default() {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                if path.starts_with(&workspace_root) {
                    sources.push(path);
                } else {
                    package.push(path);
                }
            }
        }

        for path in toml_schema.include_dirs.unwrap_or_default() {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                include_dirs.push(path);
            }
        }

        for path in toml_schema.libraries {
            let path = workspace_root.absolutize(path);
            if exclude.iter().all(|excluded| !path.starts_with(excluded)) {
                package.push(path);
            }
        }

        sort_and_remove_subfolders(&mut sources);
        sort_and_remove_subfolders(&mut include_dirs);
        sort_and_remove_subfolders(&mut package);

        if sources.is_empty() && !sources_was_configured {
            sources.push(workspace_root.clone());
        }
        if include_dirs.is_empty() && !include_dirs_was_configured {
            include_dirs = sources.clone();
        }

        Ok(TomlWorkspace {
            top_modules,
            workspace_root,
            macro_defs,
            sources,
            include_dirs,
            exclude,
            package,
            is_lib,
            has_manifest: true,
        })
    }

    pub fn from_unconfigured_root(path: &AbsPathBuf, is_lib: bool) -> Self {
        Self {
            top_modules: Vec::new(),
            workspace_root: path.clone(),
            macro_defs: MacroDef::default(),
            sources: vec![path.clone()],
            include_dirs: vec![path.clone()],
            exclude: vec![],
            package: vec![],
            is_lib,
            has_manifest: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use utils::test_support::TestDir;

    use super::*;

    #[test]
    fn test_de_macros() {
        let toml = r#"
top_modules = ["main"]
defines = [
    "foo",
    "bar",
    "FOO=bar",
    "BAR=foo",
    "BAZ=foo bar",
    "eqwe=123",
]
        "#;
        let toml_schema: TomlManifestSchema = toml::from_str(toml).unwrap();
        assert_eq!(toml_schema.top_modules, ["main"]);
        let mut macros = FxHashSet::default();
        macros.insert(MacroAtom::Flag("foo".into()));
        macros.insert(MacroAtom::Flag("bar".into()));
        macros.insert(MacroAtom::KeyValue { key: "FOO".into(), value: "bar".into() });
        macros.insert(MacroAtom::KeyValue { key: "BAR".into(), value: "foo".into() });
        macros.insert(MacroAtom::KeyValue { key: "BAZ".into(), value: "foo bar".into() });
        macros.insert(MacroAtom::KeyValue { key: "eqwe".into(), value: "123".into() });
        assert_eq!(toml_schema.defines, MacroDef { macros });
    }

    #[test]
    fn macro_predefines_are_stable() {
        let toml = r#"
defines = [
    "BAR=foo",
    "FOO",
]
        "#;
        let toml_schema: TomlManifestSchema = toml::from_str(toml).unwrap();
        assert_eq!(toml_schema.defines.to_predefine_strings(), ["BAR=foo", "FOO"]);
    }

    #[test]
    fn configured_empty_sources_do_not_default_to_workspace_root() {
        let root = TestDir::new("empty-sources");
        let manifest = root.write("vizsla_config.toml", "sources = []\n");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.sources.is_empty());
    }

    #[test]
    fn excluded_configured_sources_do_not_default_to_workspace_root() {
        let root = TestDir::new("excluded-sources");
        root.create_dir_all("rtl");
        let manifest =
            root.write("vizsla_config.toml", "sources = [\"rtl\"]\nexclude = [\"rtl\"]\n");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.sources.is_empty());
    }

    #[test]
    fn configured_empty_include_dirs_do_not_default_to_sources() {
        let root = TestDir::new("empty-include-dirs");
        root.create_dir_all("rtl");
        let manifest = root.write("vizsla_config.toml", "sources = [\"rtl\"]\ninclude_dirs = []\n");

        let workspace = TomlWorkspace::load_from_file(&manifest, false).unwrap();

        assert!(workspace.include_dirs.is_empty());
    }
}
