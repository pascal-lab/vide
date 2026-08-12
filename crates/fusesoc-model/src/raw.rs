//! Typed serde model for CAPI2 `.core` files.
//!
//! This model mirrors the official CAPI2 JSON schema but uses Rust types.
//! Fields that Vide does not execute (generators, scripts, vpi, provider) are
//! preserved so they can be detected and reported, rather than silently
//! dropped by `deny_unknown_fields`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Top-level CAPI2 core file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Core {
    /// VLNV identifier (e.g. `vendor:library:name:version`).
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub license: Option<License>,
    #[serde(default)]
    pub filesets: IndexMap<String, Fileset>,
    #[serde(default)]
    pub targets: IndexMap<String, Target>,
    #[serde(default)]
    pub parameters: IndexMap<String, Parameter>,
    #[serde(default)]
    pub provider: Option<Provider>,
    #[serde(default)]
    pub generate: IndexMap<String, GenerateInstance>,
    #[serde(default)]
    pub generators: IndexMap<String, Generator>,
    #[serde(default)]
    pub scripts: IndexMap<String, Script>,
    #[serde(default)]
    pub vpi: IndexMap<String, Vpi>,
    /// Virtual cores provided by this core (VLNV list).
    #[serde(default, rename = "virtual")]
    pub virtuals: Vec<String>,
    #[serde(default)]
    pub mapping: IndexMap<String, String>,
}

/// License can be an SPDX string or a custom {name, text} object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum License {
    Spdx(String),
    Custom { name: String, text: String },
}

/// A fileset — a named group of files with optional dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fileset {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "files")]
    pub files: Vec<FileEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "files_append")]
    pub files_append: Vec<FileEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "depend")]
    pub depend: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "depend_append")]
    pub depend_append: Vec<String>,
}

/// A file entry — either a bare path string or a {path: attributes} object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileEntry {
    Path(String),
    WithAttributes(IndexMap<String, FileAttributes>),
}

impl FileEntry {
    /// Return the file path (the single map key for `WithAttributes`, or the
    /// string for `Path`).
    pub fn path(&self) -> &str {
        match self {
            FileEntry::Path(p) => p,
            FileEntry::WithAttributes(map) => {
                map.keys().next().expect("file entry map must have one key")
            }
        }
    }

    /// Return the file attributes if present.
    pub fn attributes(&self) -> Option<&FileAttributes> {
        match self {
            FileEntry::Path(_) => None,
            FileEntry::WithAttributes(map) => map.values().next(),
        }
    }
}

/// Per-file attributes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileAttributes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub define: Option<IndexMap<String, FileDefineValue>>,
    #[serde(default)]
    pub is_include_file: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub include_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copyto: Option<String>,
}

/// Define values can be string, number, or boolean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileDefineValue {
    Str(String),
    Int(i64),
    Bool(bool),
}

/// A build target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_tool: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filesets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", rename = "filesets_append")]
    pub filesets_append: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub generate: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hooks: Option<Hooks>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vpi: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
    /// Toplevel can be a single string or a list.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "deserialize_toplevel"
    )]
    pub toplevel: Vec<String>,
}

impl Target {
    /// Normalize toplevel to a list (FuseSoC accepts scalar or list).
    pub fn top_modules(&self) -> Vec<String> {
        self.toplevel.clone()
    }
}

/// Deserialize a toplevel field that may be a string or a list of strings.
fn deserialize_toplevel<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value = Option::<serde_yaml_ng::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(Vec::new()),
        Some(serde_yaml_ng::Value::String(s)) => Ok(vec![s]),
        Some(serde_yaml_ng::Value::Sequence(seq)) => seq
            .into_iter()
            .map(|v| {
                if let serde_yaml_ng::Value::String(s) = v {
                    Ok(s)
                } else {
                    Err(serde::de::Error::custom("toplevel list items must be strings"))
                }
            })
            .collect(),
        Some(_) => Err(serde::de::Error::custom("toplevel must be a string or list of strings")),
    }
}

/// Target hooks (pre_build, post_build, pre_run, post_run).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hooks {
    #[serde(default)]
    pub pre_build: Vec<String>,
    #[serde(default)]
    pub post_build: Vec<String>,
    #[serde(default)]
    pub pre_run: Vec<String>,
    #[serde(default)]
    pub post_run: Vec<String>,
}

/// A parameter declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameter {
    pub datatype: String,
    pub paramtype: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<ParameterValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

/// Parameter default can be bool, string, or number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    Bool(bool),
    Int(i64),
    Real(String),
    Str(String),
}

/// Core provider — defines where the core is fetched from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub name: ProviderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patches: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cachable: Option<bool>,
}

/// Known provider kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    #[serde(rename = "github")]
    Github,
    #[serde(rename = "git")]
    Git,
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "opencores")]
    Opencores,
    #[serde(rename = "svn")]
    Svn,
    #[serde(rename = "url")]
    Url,
    #[serde(untagged)]
    Other(String),
}

/// A generate instance — a parameterized invocation of a generator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerateInstance {
    pub generator: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(default)]
    pub parameters: IndexMap<String, serde_yaml_ng::Value>,
}

/// A generator definition — a program that produces FuseSoC cores.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generator {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interpreter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_input_parameters: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
}

/// A build script (hook).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    #[serde(default)]
    pub cmd: Vec<String>,
    #[serde(default)]
    pub filesets: Vec<String>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
}

/// VPI library definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vpi {
    #[serde(default)]
    pub filesets: Vec<String>,
    #[serde(default)]
    pub libs: Vec<String>,
}
