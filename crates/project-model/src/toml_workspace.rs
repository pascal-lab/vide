use std::sync::LazyLock;

use anyhow::Context;
use const_format::formatcp;
use regex::Regex;
use rustc_hash::FxHashSet;
use serde::Deserialize;
use smol_str::SmolStr;
use utils::paths::{AbsPathBuf, Utf8PathBuf};

use crate::macro_def::{MacroAtom, MacroDef};

const IDENTIFIER_RE: &str = r"[a-zA-Z_][a-zA-Z0-9$_]*|\\\S* ";
#[cfg(feature = "manifest-schema")]
const MACRO_DEFINITION_SCHEMA_RE: &str = r"^(?:[A-Za-z_][A-Za-z0-9$_]*|\\\S* )(?:=.*)?$";
#[cfg(feature = "manifest-schema")]
pub const TOML_MANIFEST_SCHEMA_VERSION: &str = "v1";
#[cfg(feature = "manifest-schema")]
pub const TOML_MANIFEST_SCHEMA_PATH: &str =
    formatcp!("/vizsla/schemas/{TOML_MANIFEST_SCHEMA_VERSION}/vizsla.schema.json");
#[cfg(feature = "manifest-schema")]
pub const TOML_MANIFEST_SCHEMA_URL: &str =
    formatcp!("https://pascal-lab.github.io{TOML_MANIFEST_SCHEMA_PATH}");

static IDENT_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})$")));
static KV_RE: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(formatcp!("^({IDENTIFIER_RE})=(.*)$")));

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "manifest-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    feature = "manifest-schema",
    schemars(
        title = "Vizsla project manifest",
        description = "Project manifest for the Vizsla Verilog/SystemVerilog language server.",
        extend("$id" = TOML_MANIFEST_SCHEMA_URL, "x-tombi-table-keys-order" = "schema")
    )
)]
struct TomlManifestSchema {
    /// Top-level module names for the compilation profile.
    #[serde(default)]
    #[cfg_attr(
        feature = "manifest-schema",
        schemars(
            description = "Top-level module names for the compilation profile.",
            extend("examples" = [["top"]])
        )
    )]
    pub top_modules: Vec<String>,
    /// Predefined macros. Use NAME or NAME=value strings.
    #[serde(deserialize_with = "de_macros", default)]
    #[cfg_attr(
        feature = "manifest-schema",
        schemars(
            description = "Predefined macros. Use NAME or NAME=value strings.",
            with = "Vec::<String>",
            default = "empty_string_vec",
            inner(regex(pattern = MACRO_DEFINITION_SCHEMA_RE)),
            extend("examples" = [["SYNTHESIS", "DATA_WIDTH=32"]])
        )
    )]
    pub defines: MacroDef,
    /// Workspace-relative shell glob patterns for source files to scan. Omitted
    /// sources do not scan the workspace root.
    #[serde(default)]
    #[cfg_attr(
        feature = "manifest-schema",
        schemars(
            description = "Workspace-relative shell glob patterns for source files to scan. Omitted sources do not scan the workspace root.",
            with = "Vec::<String>",
            default = "empty_string_vec",
            extend("examples" = [["rtl/**", "ip/**/*.sv"]])
        )
    )]
    pub sources: Option<Vec<String>>,
    /// Include search directories. When omitted, Vizsla uses the scan roots
    /// inferred from sources as include directories.
    #[serde(default)]
    #[cfg_attr(
        feature = "manifest-schema",
        schemars(
            description = "Include search directories. When omitted, Vizsla uses the scan roots inferred from sources as include directories.",
            with = "Vec::<String>",
            default = "empty_string_vec",
            extend("examples" = [["include", "rtl"]])
        )
    )]
    pub include_dirs: Option<Vec<Utf8PathBuf>>,
    /// External library or dependency workspace paths.
    #[serde(default)]
    #[cfg_attr(
        feature = "manifest-schema",
        schemars(
            description = "External library or dependency workspace paths.",
            with = "Vec::<String>",
            default = "empty_string_vec",
            extend("examples" = [["../common_cells"]])
        )
    )]
    pub libraries: Vec<Utf8PathBuf>,
    /// Workspace-relative shell glob patterns to remove from loaded files.
    #[serde(default)]
    #[cfg_attr(
        feature = "manifest-schema",
        schemars(
            description = "Workspace-relative shell glob patterns to remove from loaded files.",
            with = "Vec::<String>",
            default = "empty_string_vec",
            extend("examples" = [["build/**", "sim/work/**", "**/*_bb.v"]])
        )
    )]
    pub exclude: Vec<String>,
}

#[cfg(feature = "manifest-schema")]
fn empty_string_vec() -> Vec<String> {
    Vec::new()
}

#[cfg(feature = "manifest-schema")]
pub fn generated_toml_manifest_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(TomlManifestSchema)).unwrap()
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
    pub source_patterns: Vec<String>,
    pub include_dirs: Option<Vec<AbsPathBuf>>,
    pub libraries: Vec<AbsPathBuf>,
    pub exclude_patterns: Vec<String>,
}

impl TomlWorkspace {
    pub fn load_from_file(toml: &AbsPathBuf) -> anyhow::Result<Self> {
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

        let include_dirs = toml_schema.include_dirs.map(|paths| {
            paths.into_iter().map(|path| workspace_root.absolutize(path)).collect::<Vec<_>>()
        });
        let source_patterns = toml_schema.sources.unwrap_or_default();
        let libraries = toml_schema
            .libraries
            .into_iter()
            .map(|path| workspace_root.absolutize(path))
            .collect::<Vec<_>>();
        let exclude_patterns = toml_schema.exclude;

        Ok(TomlWorkspace {
            top_modules,
            workspace_root,
            macro_defs,
            source_patterns,
            include_dirs,
            libraries,
            exclude_patterns,
        })
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
    fn empty_manifest_uses_syntax_only_default() {
        let root = TestDir::new("empty-manifest");
        let manifest = root.write("vizsla_config.toml", "");

        let workspace = TomlWorkspace::load_from_file(&manifest).unwrap();

        assert!(workspace.source_patterns.is_empty());
        assert_eq!(workspace.include_dirs, None);
        assert!(workspace.libraries.is_empty());
        assert!(workspace.exclude_patterns.is_empty());
    }

    #[test]
    fn configured_empty_sources_do_not_default_to_workspace_root() {
        let root = TestDir::new("empty-sources");
        let manifest = root.write("vizsla_config.toml", "sources = []\n");

        let workspace = TomlWorkspace::load_from_file(&manifest).unwrap();

        assert!(workspace.source_patterns.is_empty());
    }

    #[test]
    fn keeps_source_and_exclude_globs_relative_to_workspace() {
        let root = TestDir::new("manifest-source-exclude-globs");
        root.create_dir_all("rtl");
        let manifest = root.write(
            "vizsla_config.toml",
            "sources = [\"rtl/**\"]\nexclude = [\"build/**\", \"**/*_bb.v\"]\n",
        );

        let workspace = TomlWorkspace::load_from_file(&manifest).unwrap();

        assert_eq!(workspace.source_patterns, ["rtl/**"]);
        assert_eq!(workspace.exclude_patterns, ["build/**", "**/*_bb.v"]);
    }

    #[test]
    fn configured_empty_include_dirs_do_not_default_to_sources() {
        let root = TestDir::new("empty-include-dirs");
        root.create_dir_all("rtl");
        let manifest =
            root.write("vizsla_config.toml", "sources = [\"rtl/**\"]\ninclude_dirs = []\n");

        let workspace = TomlWorkspace::load_from_file(&manifest).unwrap();

        assert_eq!(workspace.include_dirs, Some(Vec::new()));
    }

    #[test]
    fn parses_paths_as_absolute_paths() {
        let root = TestDir::new("manifest-paths");
        let manifest = root.write(
            "vizsla_config.toml",
            r#"include_dirs = ["include"]
libraries = ["../pkg"]
"#,
        );

        let workspace = TomlWorkspace::load_from_file(&manifest).unwrap();

        assert_eq!(workspace.include_dirs, Some(vec![root.join("include")]));
        assert_eq!(workspace.libraries, [root.join("../pkg")]);
    }
}
