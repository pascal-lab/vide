use std::{ops::Range, sync::LazyLock};

use const_format::formatcp;
use regex::Regex;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use smol_str::SmolStr;
use utils::paths::Utf8PathBuf;

use crate::macro_def::{MacroAtom, MacroDef};

const IDENTIFIER_RE: &str = r"[a-zA-Z_][a-zA-Z0-9$_]*|\\\S* ";
static IDENT_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})$")));
static KV_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})=(.*)$")));

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TomlManifestSchema {
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
pub struct TomlManifestDiagnostic {
    pub range: Option<Range<usize>>,
    pub message: String,
}

pub fn toml_manifest_diagnostics(text: &str) -> Vec<TomlManifestDiagnostic> {
    match toml::from_str::<TomlManifestSchema>(text) {
        Ok(_) => Vec::new(),
        Err(error) => {
            vec![TomlManifestDiagnostic { range: error.span(), message: error.to_string() }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_macro_definitions() {
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
    fn diagnostics_report_schema_errors() {
        let diagnostics = toml_manifest_diagnostics("source = [\"rtl\"]\n");

        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("unknown field"));
    }
}
