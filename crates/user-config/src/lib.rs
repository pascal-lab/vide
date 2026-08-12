use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, Error as _},
};

const DEFAULT_QIHE_RUN_ARGS: &[&str] = &["-g", "std"];
const DEFAULT_SLANG_WIDTH_WARNINGS: &[&str] =
    &["width-expand", "width-trunc", "port-width-expand", "port-width-trunc"];
const USER_CONFIG_SCHEMA_FIELD: &str = "$schema";
const USER_CONFIG_SCHEMA_URL: &str =
    "https://vide.pascal-lab.net/schemas/v1/user-config.schema.json";

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum FilesWatcherDef {
    #[default]
    Client,
    Notify,
    Server,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ScopeVisibility {
    Public,
    #[default]
    Private,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum FormatterProviderUserConfig {
    #[default]
    Verible,
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default, schemars::JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticsUpdateUserConfig {
    OnType,
    #[default]
    OnSave,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticRuleSeverityUserConfig {
    Ignore,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(
    title = "Vide language server user configuration",
    description = "Initialization options and dynamic configuration accepted by the Vide language server. These options are useful for editors that configure LSP servers directly, such as Neovim and Emacs.",
    deny_unknown_fields
)]
pub struct UserConfig {
    pub files: FilesUserConfig,
    pub workspace: WorkspaceUserConfig,
    pub scope: ScopeUserConfig,
    pub references: ReferencesUserConfig,
    pub formatter: FormatterUserConfig,
    pub formatting: FormattingUserConfig,
    #[serde(rename = "inlayHints")]
    pub inlay_hints: InlayHintsUserConfig,
    pub lens: LensUserConfig,
    pub semantic: SemanticUserConfig,
    pub diagnostics: DiagnosticsUserConfig,
    pub signature: SignatureUserConfig,
    pub qihe: QiheUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FilesUserConfig {
    /// These directories will be ignored. They are relative to the workspace
    /// root, and globs are not supported. You may also need to add the folders
    /// to VS Code's `files.watcherExclude`.
    #[serde(rename = "excludeDirs")]
    #[schemars(
        description = "Workspace-relative directories ignored by Vide. Globs are not supported.",
        with = "Vec::<String>",
        default = "empty_string_vec"
    )]
    pub exclude_dirs: Vec<Utf8PathBuf>,
    /// Controls file watching.
    #[schemars(
        description = "Controls how Vide watches project files.",
        default = "FilesWatcherDef::default"
    )]
    pub watcher: FilesWatcherDef,
}

impl Default for FilesUserConfig {
    fn default() -> Self {
        Self { exclude_dirs: Vec::new(), watcher: FilesWatcherDef::Client }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct WorkspaceUserConfig {
    pub auto: WorkspaceAutoUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct WorkspaceAutoUserConfig {
    /// Automatically refresh project info on toml changes.
    #[schemars(
        description = "Automatically refresh project information when project manifests change.",
        default = "default_true"
    )]
    pub reload: bool,
}

impl Default for WorkspaceAutoUserConfig {
    fn default() -> Self {
        Self { reload: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ScopeUserConfig {
    /// If true, symbols within a scope, except for ports, are private to other
    /// scopes.
    #[schemars(
        description = "Controls whether symbols inside scopes, except ports, are visible outside those scopes.",
        default = "ScopeVisibility::default"
    )]
    pub visibility: ScopeVisibility,
}

impl Default for ScopeUserConfig {
    fn default() -> Self {
        Self { visibility: ScopeVisibility::Private }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct ReferencesUserConfig {
    #[serde(rename = "includeDeclaration")]
    #[schemars(
        description = "Include declarations when finding references.",
        default = "default_true"
    )]
    pub include_declaration: bool,
}

impl Default for ReferencesUserConfig {
    fn default() -> Self {
        Self { include_declaration: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FormatterUserConfig {
    #[schemars(
        description = "Formatter backend used by Vide.",
        default = "FormatterProviderUserConfig::default"
    )]
    pub provider: FormatterProviderUserConfig,
    #[schemars(
        description = "Path to verible-verilog-format when formatter.provider is verible. Use null to find it on PATH.",
        with = "Option::<String>"
    )]
    pub path: Option<Utf8PathBuf>,
    #[schemars(
        description = "Arguments passed to verible-verilog-format when formatter.provider is verible.",
        default = "default_formatter_args"
    )]
    pub args: Vec<String>,
}

impl Default for FormatterUserConfig {
    fn default() -> Self {
        Self {
            provider: FormatterProviderUserConfig::Verible,
            path: None,
            args: vec!["--failsafe_success=false".to_owned()],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FormattingUserConfig {
    pub on: FormattingOnUserConfig,
    pub r#in: FormattingInUserConfig,
    pub indent: FormattingIndentUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FormattingOnUserConfig {
    #[schemars(
        description = "Enable formatting behavior when pressing Enter.",
        default = "default_true"
    )]
    pub enter: bool,
}

impl Default for FormattingOnUserConfig {
    fn default() -> Self {
        Self { enter: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FormattingInUserConfig {
    #[schemars(description = "Enable formatting inside comments.", default = "default_true")]
    pub comments: bool,
}

impl Default for FormattingInUserConfig {
    fn default() -> Self {
        Self { comments: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct FormattingIndentUserConfig {
    #[schemars(
        description = "Fallback indentation width used when editor formatting options are unavailable.",
        default = "default_indent_width",
        range(min = 0)
    )]
    pub width: usize,
}

impl Default for FormattingIndentUserConfig {
    fn default() -> Self {
        Self { width: 4 }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InlayHintsUserConfig {
    pub port: InlayHintsPortUserConfig,
    pub parameter: InlayHintsParameterUserConfig,
    pub r#macro: InlayHintsMacroUserConfig,
    pub end: InlayHintsEndUserConfig,
    pub system_call: InlayHintsSystemCallUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InlayHintsPortUserConfig {
    pub connection: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InlayHintsParameterUserConfig {
    pub assignment: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InlayHintsMacroUserConfig {
    pub argument: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InlayHintsEndUserConfig {
    pub structure: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct InlayHintsSystemCallUserConfig {
    pub call: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct LensUserConfig {
    pub instantiations: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SemanticUserConfig {
    pub tokens: SemanticTokensUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SemanticTokensUserConfig {
    pub port: SemanticTokensPortUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SemanticTokensPortUserConfig {
    pub clk: SemanticTokensClockUserConfig,
    pub input: SemanticTokensInputUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SemanticTokensClockUserConfig {
    pub rst: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SemanticTokensInputUserConfig {
    pub output: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DiagnosticsUserConfig {
    #[schemars(description = "Enable diagnostics.", default = "default_true")]
    pub enable: bool,
    #[schemars(
        description = "Controls when diagnostics are refreshed.",
        default = "DiagnosticsUpdateUserConfig::default"
    )]
    pub update: DiagnosticsUpdateUserConfig,
    pub parse: DiagnosticsPhaseUserConfig,
    pub semantic: DiagnosticsPhaseUserConfig,
    pub slang: SlangDiagnosticsUserConfig,
}

impl Default for DiagnosticsUserConfig {
    fn default() -> Self {
        Self {
            enable: true,
            update: DiagnosticsUpdateUserConfig::default(),
            parse: DiagnosticsPhaseUserConfig::default(),
            semantic: DiagnosticsPhaseUserConfig::default(),
            slang: SlangDiagnosticsUserConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DiagnosticsPhaseUserConfig {
    #[schemars(default = "default_true")]
    pub enable: bool,
}

impl Default for DiagnosticsPhaseUserConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SlangDiagnosticsUserConfig {
    #[schemars(
        description = "Additional slang warning groups or aliases to enable.",
        default = "default_slang_width_warnings"
    )]
    pub warnings: Vec<String>,
    #[schemars(description = "Per-diagnostic severity overrides.")]
    pub rules: Vec<DiagnosticRuleUserConfig>,
}

impl Default for SlangDiagnosticsUserConfig {
    fn default() -> Self {
        Self { warnings: default_slang_width_warnings(), rules: Vec::new() }
    }
}

fn default_slang_width_warnings() -> Vec<String> {
    DEFAULT_SLANG_WIDTH_WARNINGS.iter().map(|warning| (*warning).to_owned()).collect()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct DiagnosticRuleUserConfig {
    #[schemars(regex(
        pattern = "^(code:[0-9]+:[0-9]+|option:.+|group:.+|source:(parse|semantic))$"
    ))]
    pub selector: String,
    pub severity: DiagnosticRuleSeverityUserConfig,
    #[serde(default)]
    pub force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SignatureUserConfig {
    pub help: SignatureHelpUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SignatureHelpUserConfig {
    pub params: SignatureHelpParamsUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct SignatureHelpParamsUserConfig {
    #[schemars(description = "Only show parameter signature help.")]
    pub only: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct QiheUserConfig {
    #[schemars(
        description = "Command used to invoke Qihe. Use null to resolve the platform default.",
        default = "default_qihe_command_setting"
    )]
    pub command: Option<String>,
    #[serde(rename = "autoConfigureArgsFromManifest")]
    #[schemars(
        description = "Automatically add Qihe compile mode and forwarded slang options from the Vide project manifest.",
        default = "default_true"
    )]
    pub auto_configure_args_from_manifest: bool,
    #[serde(rename = "compileArgs")]
    #[schemars(
        description = "Arguments inserted after qihe compile.",
        default = "empty_string_vec"
    )]
    pub compile_args: Vec<String>,
    #[serde(rename = "runArgs")]
    #[schemars(
        description = "Arguments inserted after qihe run.",
        default = "default_qihe_run_args"
    )]
    pub run_args: Vec<String>,
}

impl Default for QiheUserConfig {
    fn default() -> Self {
        Self {
            command: None,
            auto_configure_args_from_manifest: true,
            compile_args: Vec::new(),
            run_args: DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_owned()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(deny_unknown_fields)]
pub struct EnableUserConfig {
    #[schemars(default = "default_true")]
    pub enable: bool,
}

impl Default for EnableUserConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

fn default_true() -> bool {
    true
}

fn empty_string_vec() -> Vec<String> {
    Vec::new()
}

fn default_formatter_args() -> Vec<String> {
    vec!["--failsafe_success=false".to_owned()]
}

fn default_indent_width() -> usize {
    4
}

fn default_qihe_command_setting() -> Option<String> {
    None
}

fn default_qihe_run_args() -> Vec<String> {
    DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_owned()).collect()
}

pub fn generated_user_config_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(UserConfig))
        .expect("user config schema should serialize");
    if let Some(root) = schema.as_object_mut() {
        root.insert("$id".to_owned(), serde_json::json!(USER_CONFIG_SCHEMA_URL));
        root.insert("x-vide-config-kind".to_owned(), serde_json::json!("user"));
        if let Some(properties) = root.get_mut("properties").and_then(|it| it.as_object_mut()) {
            properties.insert(
                USER_CONFIG_SCHEMA_FIELD.to_owned(),
                serde_json::json!({
                    "type": "string",
                    "description": "JSON schema URL used by editors for completion and validation. Vide ignores this field.",
                }),
            );
        }
    }
    schema
}

pub struct ConfigSettingMeta {
    pub path: &'static [&'static str],
    pub vscode_key: &'static str,
    pub docs_group: &'static str,
    pub description_key: &'static str,
    pub markdown_description_key: Option<&'static str>,
    pub enum_descriptions: &'static [(&'static str, &'static str)],
    pub exposed_in_vscode: bool,
    default: ConfigSettingDefault,
    schema: ConfigSettingSchema,
}

#[derive(Clone, Copy)]
enum ConfigSettingDefault {
    Bool(bool),
    String(&'static str),
    Null,
    StringArray(&'static [&'static str]),
    Usize(usize),
}

#[derive(Clone, Copy)]
enum ConfigSettingSchema {
    Boolean,
    StringOrNull,
    StringArray,
    Integer { minimum: usize },
    Enum { values: &'static [&'static str] },
    DiagnosticRules,
}

impl ConfigSettingMeta {
    fn vscode_section(&self) -> &'static str {
        self.vscode_key.strip_prefix("vide.").unwrap_or(self.vscode_key)
    }

    fn default_json(&self) -> serde_json::Value {
        match self.default {
            ConfigSettingDefault::Bool(value) => serde_json::json!(value),
            ConfigSettingDefault::String(value) => serde_json::json!(value),
            ConfigSettingDefault::Null => serde_json::Value::Null,
            ConfigSettingDefault::StringArray(values) => serde_json::json!(values),
            ConfigSettingDefault::Usize(value) => serde_json::json!(value),
        }
    }

    fn package_property(&self) -> serde_json::Value {
        let mut property = serde_json::Map::new();
        self.insert_schema(&mut property);
        property.insert("default".to_owned(), self.default_json());

        if let Some(markdown_key) = self.markdown_description_key {
            property.insert(
                "markdownDescription".to_owned(),
                serde_json::json!(format!("%{markdown_key}%")),
            );
        } else {
            property.insert(
                "description".to_owned(),
                serde_json::json!(format!("%{}%", self.description_key)),
            );
        }

        if !self.enum_descriptions.is_empty() {
            let descriptions = self
                .enum_descriptions
                .iter()
                .map(|(_, key)| format!("%{key}%"))
                .collect::<Vec<_>>();
            property.insert("enumDescriptions".to_owned(), serde_json::json!(descriptions));
        }

        serde_json::Value::Object(property)
    }

    fn insert_schema(&self, property: &mut serde_json::Map<String, serde_json::Value>) {
        match self.schema {
            ConfigSettingSchema::Boolean => {
                property.insert("type".to_owned(), serde_json::json!("boolean"));
            }
            ConfigSettingSchema::StringOrNull => {
                property.insert("type".to_owned(), serde_json::json!(["string", "null"]));
            }
            ConfigSettingSchema::StringArray => {
                property.insert("type".to_owned(), serde_json::json!("array"));
                property.insert("items".to_owned(), serde_json::json!({ "type": "string" }));
            }
            ConfigSettingSchema::Integer { minimum } => {
                property.insert("type".to_owned(), serde_json::json!("integer"));
                property.insert("minimum".to_owned(), serde_json::json!(minimum));
            }
            ConfigSettingSchema::Enum { values } => {
                property.insert("type".to_owned(), serde_json::json!("string"));
                property.insert("enum".to_owned(), serde_json::json!(values));
            }
            ConfigSettingSchema::DiagnosticRules => {
                property.insert("type".to_owned(), serde_json::json!("array"));
                property.insert(
                    "items".to_owned(),
                    serde_json::json!({
                        "type": "object",
                        "required": ["selector", "severity"],
                        "properties": {
                            "selector": {
                                "type": "string",
                                "pattern": "^(code:[0-9]+:[0-9]+|option:.+|group:.+|source:(parse|semantic))$",
                                "markdownDescription": "%configuration.diagnostics.slang.rules.selector.markdownDescription%",
                            },
                            "severity": {
                                "type": "string",
                                "enum": ["ignore", "info", "warning", "error", "fatal"],
                                "description": "%configuration.diagnostics.slang.rules.severity.description%",
                            },
                            "force": {
                                "type": "boolean",
                                "default": false,
                                "description": "%configuration.diagnostics.slang.rules.force.description%",
                            },
                        },
                        "additionalProperties": false,
                    }),
                );
            }
        }
    }
}

const FILES_WATCHER_ENUM_DESCRIPTIONS: &[(&str, &str)] = &[
    ("client", "configuration.files.watcher.enum.client"),
    ("notify", "configuration.files.watcher.enum.notify"),
    ("server", "configuration.files.watcher.enum.server"),
];

const FORMATTER_PROVIDER_ENUM_DESCRIPTIONS: &[(&str, &str)] =
    &[("verible", "configuration.formatter.provider.enum.verible")];

const USER_CONFIG_SETTINGS: &[ConfigSettingMeta] = &[
    ConfigSettingMeta {
        path: &["qihe", "command"],
        vscode_key: "vide.qihe.command",
        docs_group: "Qihe",
        description_key: "configuration.qihe.command.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Null,
        schema: ConfigSettingSchema::StringOrNull,
    },
    ConfigSettingMeta {
        path: &["qihe", "autoConfigureArgsFromManifest"],
        vscode_key: "vide.qihe.autoConfigureArgsFromManifest",
        docs_group: "Qihe",
        description_key: "configuration.qihe.autoConfigureArgsFromManifest.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["qihe", "compileArgs"],
        vscode_key: "vide.qihe.compileArgs",
        docs_group: "Qihe",
        description_key: "configuration.qihe.compileArgs.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::StringArray(&[]),
        schema: ConfigSettingSchema::StringArray,
    },
    ConfigSettingMeta {
        path: &["qihe", "runArgs"],
        vscode_key: "vide.qihe.runArgs",
        docs_group: "Qihe",
        description_key: "configuration.qihe.runArgs.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::StringArray(DEFAULT_QIHE_RUN_ARGS),
        schema: ConfigSettingSchema::StringArray,
    },
    ConfigSettingMeta {
        path: &["files", "excludeDirs"],
        vscode_key: "vide.files.excludeDirs",
        docs_group: "Files",
        description_key: "configuration.files.excludeDirs.markdownDescription",
        markdown_description_key: Some("configuration.files.excludeDirs.markdownDescription"),
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::StringArray(&[]),
        schema: ConfigSettingSchema::StringArray,
    },
    ConfigSettingMeta {
        path: &["files", "watcher"],
        vscode_key: "vide.files.watcher",
        docs_group: "Files",
        description_key: "configuration.files.watcher.description",
        markdown_description_key: None,
        enum_descriptions: FILES_WATCHER_ENUM_DESCRIPTIONS,
        exposed_in_vscode: true,
        default: ConfigSettingDefault::String("client"),
        schema: ConfigSettingSchema::Enum { values: &["client", "notify", "server"] },
    },
    ConfigSettingMeta {
        path: &["workspace", "auto", "reload"],
        vscode_key: "vide.workspace.auto.reload",
        docs_group: "Workspace",
        description_key: "configuration.workspace.auto.reload.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["scope", "visibility"],
        vscode_key: "vide.scope.visibility",
        docs_group: "Navigation",
        description_key: "configuration.scope.visibility.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::String("private"),
        schema: ConfigSettingSchema::Enum { values: &["private", "public"] },
    },
    ConfigSettingMeta {
        path: &["references", "includeDeclaration"],
        vscode_key: "vide.references.includeDeclaration",
        docs_group: "Navigation",
        description_key: "configuration.references.includeDeclaration.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["formatter", "provider"],
        vscode_key: "vide.formatter.provider",
        docs_group: "Formatting",
        description_key: "configuration.formatter.provider.description",
        markdown_description_key: None,
        enum_descriptions: FORMATTER_PROVIDER_ENUM_DESCRIPTIONS,
        exposed_in_vscode: true,
        default: ConfigSettingDefault::String("verible"),
        schema: ConfigSettingSchema::Enum { values: &["verible"] },
    },
    ConfigSettingMeta {
        path: &["formatter", "path"],
        vscode_key: "vide.formatter.path",
        docs_group: "Formatting",
        description_key: "configuration.formatter.path.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Null,
        schema: ConfigSettingSchema::StringOrNull,
    },
    ConfigSettingMeta {
        path: &["formatter", "args"],
        vscode_key: "vide.formatter.args",
        docs_group: "Formatting",
        description_key: "configuration.formatter.args.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::StringArray(&["--failsafe_success=false"]),
        schema: ConfigSettingSchema::StringArray,
    },
    ConfigSettingMeta {
        path: &["formatting", "on", "enter"],
        vscode_key: "vide.formatting.on.enter",
        docs_group: "Formatting",
        description_key: "configuration.formatting.on.enter.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["formatting", "in", "comments"],
        vscode_key: "vide.formatting.in.comments",
        docs_group: "Formatting",
        description_key: "configuration.formatting.in.comments.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["formatting", "indent", "width"],
        vscode_key: "vide.formatting.indent.width",
        docs_group: "Formatting",
        description_key: "configuration.formatting.indent.width.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Usize(4),
        schema: ConfigSettingSchema::Integer { minimum: 0 },
    },
    ConfigSettingMeta {
        path: &["inlayHints", "port", "connection", "enable"],
        vscode_key: "vide.inlayHints.port.connection.enable",
        docs_group: "Annotations",
        description_key: "configuration.inlayHints.port.connection.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["inlayHints", "parameter", "assignment", "enable"],
        vscode_key: "vide.inlayHints.parameter.assignment.enable",
        docs_group: "Annotations",
        description_key: "configuration.inlayHints.parameter.assignment.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["inlayHints", "macro", "argument", "enable"],
        vscode_key: "vide.inlayHints.macro.argument.enable",
        docs_group: "Annotations",
        description_key: "configuration.inlayHints.macro.argument.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["inlayHints", "end", "structure", "enable"],
        vscode_key: "vide.inlayHints.end.structure.enable",
        docs_group: "Annotations",
        description_key: "configuration.inlayHints.end.structure.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["inlayHints", "systemCall", "call", "enable"],
        vscode_key: "vide.inlayHints.systemCall.call.enable",
        docs_group: "Annotations",
        description_key: "configuration.inlayHints.systemCall.call.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["lens", "instantiations", "enable"],
        vscode_key: "vide.lens.instantiations.enable",
        docs_group: "Annotations",
        description_key: "configuration.lens.instantiations.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["semantic", "tokens", "port", "clk", "rst", "enable"],
        vscode_key: "vide.semantic.tokens.port.clk.rst.enable",
        docs_group: "Semantic Highlighting",
        description_key: "configuration.semantic.tokens.port.clk.rst.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["semantic", "tokens", "port", "input", "output", "enable"],
        vscode_key: "vide.semantic.tokens.port.input.output.enable",
        docs_group: "Semantic Highlighting",
        description_key: "configuration.semantic.tokens.port.input.output.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["diagnostics", "enable"],
        vscode_key: "vide.diagnostics.enable",
        docs_group: "Diagnostics",
        description_key: "configuration.diagnostics.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["diagnostics", "update"],
        vscode_key: "vide.diagnostics.update",
        docs_group: "Diagnostics",
        description_key: "configuration.diagnostics.update.markdownDescription",
        markdown_description_key: Some("configuration.diagnostics.update.markdownDescription"),
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::String("onSave"),
        schema: ConfigSettingSchema::Enum { values: &["onSave", "onType"] },
    },
    ConfigSettingMeta {
        path: &["diagnostics", "parse", "enable"],
        vscode_key: "vide.diagnostics.parse.enable",
        docs_group: "Diagnostics",
        description_key: "configuration.diagnostics.parse.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["diagnostics", "semantic", "enable"],
        vscode_key: "vide.diagnostics.semantic.enable",
        docs_group: "Diagnostics",
        description_key: "configuration.diagnostics.semantic.enable.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(true),
        schema: ConfigSettingSchema::Boolean,
    },
    ConfigSettingMeta {
        path: &["diagnostics", "slang", "warnings"],
        vscode_key: "vide.diagnostics.slang.warnings",
        docs_group: "Diagnostics",
        description_key: "configuration.diagnostics.slang.warnings.markdownDescription",
        markdown_description_key: Some(
            "configuration.diagnostics.slang.warnings.markdownDescription",
        ),
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::StringArray(DEFAULT_SLANG_WIDTH_WARNINGS),
        schema: ConfigSettingSchema::StringArray,
    },
    ConfigSettingMeta {
        path: &["diagnostics", "slang", "rules"],
        vscode_key: "vide.diagnostics.slang.rules",
        docs_group: "Diagnostics",
        description_key: "configuration.diagnostics.slang.rules.markdownDescription",
        markdown_description_key: Some("configuration.diagnostics.slang.rules.markdownDescription"),
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::StringArray(&[]),
        schema: ConfigSettingSchema::DiagnosticRules,
    },
    ConfigSettingMeta {
        path: &["signature", "help", "params", "only"],
        vscode_key: "vide.signature.help.params.only",
        docs_group: "Signature Help",
        description_key: "configuration.signature.help.params.only.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::Bool(false),
        schema: ConfigSettingSchema::Boolean,
    },
];

pub fn generated_vscode_package_properties() -> serde_json::Map<String, serde_json::Value> {
    USER_CONFIG_SETTINGS
        .iter()
        .filter(|setting| setting.exposed_in_vscode)
        .map(|setting| (setting.vscode_key.to_owned(), setting.package_property()))
        .collect()
}

pub fn generated_vscode_configuration_typescript() -> String {
    let mut out = String::from(
        "// Generated by `cargo xtask generate-config-artifacts`; do not edit.\n\n\
         export type GeneratedUserConfigSetting = {\n\
         \treadonly path: readonly string[];\n\
         \treadonly vscodeKey: string;\n\
         \treadonly vscodeSection: string;\n\
         \treadonly docsGroup: string;\n\
         \treadonly descriptionKey: string;\n\
         \treadonly markdownDescriptionKey: string | null;\n\
         \treadonly defaultValue: unknown;\n\
         };\n\n\
         export const USER_CONFIG_SETTINGS = [\n",
    );

    for setting in USER_CONFIG_SETTINGS {
        out.push_str("\t{\n");
        out.push_str(&format!(
            "\t\tpath: {},\n",
            serde_json::to_string(setting.path).expect("setting path should serialize")
        ));
        out.push_str(&format!(
            "\t\tvscodeKey: {},\n",
            serde_json::to_string(setting.vscode_key).expect("setting key should serialize")
        ));
        out.push_str(&format!(
            "\t\tvscodeSection: {},\n",
            serde_json::to_string(setting.vscode_section())
                .expect("setting section should serialize")
        ));
        out.push_str(&format!(
            "\t\tdocsGroup: {},\n",
            serde_json::to_string(setting.docs_group).expect("docs group should serialize")
        ));
        out.push_str(&format!(
            "\t\tdescriptionKey: {},\n",
            serde_json::to_string(setting.description_key)
                .expect("description key should serialize")
        ));
        out.push_str(&format!(
            "\t\tmarkdownDescriptionKey: {},\n",
            setting
                .markdown_description_key
                .map_or_else(|| "null".to_owned(), |key| serde_json::to_string(key).unwrap())
        ));
        out.push_str(&format!(
            "\t\tdefaultValue: {},\n",
            serde_json::to_string(&setting.default_json()).expect("default should serialize")
        ));
        out.push_str("\t},\n");
    }

    out.push_str("] as const satisfies readonly GeneratedUserConfigSetting[];\n");
    out
}

const USER_CONFIG_KNOWN_PATHS: &[&[&str]] = &[
    &["diagnostics", "enable"],
    &["diagnostics", "parse", "enable"],
    &["diagnostics", "semantic", "enable"],
    &["diagnostics", "slang", "rules"],
    &["diagnostics", "slang", "warnings"],
    &["diagnostics", "update"],
    &["files", "excludeDirs"],
    &["files", "watcher"],
    &["formatter", "args"],
    &["formatter", "path"],
    &["formatter", "provider"],
    &["formatting", "in", "comments"],
    &["formatting", "indent", "width"],
    &["formatting", "on", "enter"],
    &["inlayHints", "end", "structure", "enable"],
    &["inlayHints", "macro", "argument", "enable"],
    &["inlayHints", "parameter", "assignment", "enable"],
    &["inlayHints", "port", "connection", "enable"],
    &["inlayHints", "systemCall", "call", "enable"],
    &["lens", "instantiations", "enable"],
    &["qihe", "autoConfigureArgsFromManifest"],
    &["qihe", "command"],
    &["qihe", "compileArgs"],
    &["qihe", "runArgs"],
    &["references", "includeDeclaration"],
    &["scope", "visibility"],
    &["semantic", "tokens", "port", "clk", "rst", "enable"],
    &["semantic", "tokens", "port", "input", "output", "enable"],
    &["signature", "help", "params", "only"],
    &["workspace", "auto", "reload"],
];

#[derive(Default)]
struct UserConfigPathNode {
    children: BTreeMap<&'static str, UserConfigPathNode>,
}

impl UserConfigPathNode {
    fn insert(&mut self, path: &'static [&'static str]) {
        let mut current = self;
        for segment in path {
            current = current.children.entry(segment).or_default();
        }
    }
}

fn user_config_path_tree() -> UserConfigPathNode {
    let mut root = UserConfigPathNode::default();
    for path in USER_CONFIG_KNOWN_PATHS {
        root.insert(path);
    }
    root
}

fn escape_json_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

fn user_config_pointer(path: &[&str]) -> String {
    let mut pointer = String::new();
    for segment in path {
        pointer.push('/');
        pointer.push_str(&escape_json_pointer_segment(segment));
    }
    pointer
}

fn child_pointer(parent: &str, child: &str) -> String {
    format!("{parent}/{}", escape_json_pointer_segment(child))
}

fn config_error(message: impl std::fmt::Display) -> serde_json::Error {
    serde_json::Error::custom(message)
}

fn validate_user_config_shape(
    json: &serde_json::Value,
    node: &UserConfigPathNode,
    pointer: &str,
    error_sink: &mut Vec<(String, serde_json::Error)>,
) {
    if node.children.is_empty() {
        return;
    }

    let Some(object) = json.as_object() else {
        let display_pointer = if pointer.is_empty() { "/" } else { pointer };
        error_sink.push((display_pointer.to_owned(), config_error("expected object")));
        return;
    };

    for (key, value) in object {
        if pointer.is_empty() && key == USER_CONFIG_SCHEMA_FIELD {
            continue;
        }

        let next_pointer = child_pointer(pointer, key);
        if let Some(child) = node.children.get(key.as_str()) {
            validate_user_config_shape(value, child, &next_pointer, error_sink);
        } else {
            error_sink.push((next_pointer, config_error("unknown field")));
        }
    }
}

fn parse_user_config_field<T: DeserializeOwned>(
    json: &serde_json::Value,
    config: &mut UserConfig,
    error_sink: &mut Vec<(String, serde_json::Error)>,
    path: &[&str],
    apply: impl FnOnce(&mut UserConfig, T),
) {
    let pointer = user_config_pointer(path);
    let Some(value) = json.pointer(&pointer) else {
        return;
    };

    match serde_json::from_value(value.clone()) {
        Ok(value) => apply(config, value),
        Err(error) => error_sink.push((pointer, error)),
    }
}

fn apply_user_config_fields(
    json: &serde_json::Value,
    config: &mut UserConfig,
    error_sink: &mut Vec<(String, serde_json::Error)>,
) {
    macro_rules! field {
        ($path:expr, $ty:ty, | $cfg:ident, $value:ident | $body:expr) => {
            parse_user_config_field::<$ty>(json, config, error_sink, $path, |$cfg, $value| $body);
        };
    }

    field!(&["diagnostics", "enable"], bool, |cfg, value| cfg.diagnostics.enable = value);
    field!(&["diagnostics", "parse", "enable"], bool, |cfg, value| {
        cfg.diagnostics.parse.enable = value
    });
    field!(&["diagnostics", "semantic", "enable"], bool, |cfg, value| {
        cfg.diagnostics.semantic.enable = value
    });
    field!(&["diagnostics", "slang", "rules"], Vec<DiagnosticRuleUserConfig>, |cfg, value| {
        cfg.diagnostics.slang.rules = value
    });
    field!(&["diagnostics", "slang", "warnings"], Vec<String>, |cfg, value| {
        cfg.diagnostics.slang.warnings = value
    });
    field!(&["diagnostics", "update"], DiagnosticsUpdateUserConfig, |cfg, value| {
        cfg.diagnostics.update = value
    });
    field!(&["files", "excludeDirs"], Vec<Utf8PathBuf>, |cfg, value| {
        cfg.files.exclude_dirs = value
    });
    field!(&["files", "watcher"], FilesWatcherDef, |cfg, value| cfg.files.watcher = value);
    field!(&["formatter", "args"], Vec<String>, |cfg, value| cfg.formatter.args = value);
    field!(&["formatter", "path"], Option<Utf8PathBuf>, |cfg, value| {
        cfg.formatter.path = value
    });
    field!(&["formatter", "provider"], FormatterProviderUserConfig, |cfg, value| {
        cfg.formatter.provider = value
    });
    field!(&["formatting", "in", "comments"], bool, |cfg, value| {
        cfg.formatting.r#in.comments = value
    });
    field!(&["formatting", "indent", "width"], usize, |cfg, value| {
        cfg.formatting.indent.width = value
    });
    field!(&["formatting", "on", "enter"], bool, |cfg, value| { cfg.formatting.on.enter = value });
    field!(&["inlayHints", "end", "structure", "enable"], bool, |cfg, value| {
        cfg.inlay_hints.end.structure.enable = value
    });
    field!(&["inlayHints", "macro", "argument", "enable"], bool, |cfg, value| {
        cfg.inlay_hints.r#macro.argument.enable = value
    });
    field!(&["inlayHints", "parameter", "assignment", "enable"], bool, |cfg, value| {
        cfg.inlay_hints.parameter.assignment.enable = value
    });
    field!(&["inlayHints", "port", "connection", "enable"], bool, |cfg, value| {
        cfg.inlay_hints.port.connection.enable = value
    });
    field!(&["inlayHints", "systemCall", "call", "enable"], bool, |cfg, value| {
        cfg.inlay_hints.system_call.call.enable = value
    });
    field!(&["lens", "instantiations", "enable"], bool, |cfg, value| {
        cfg.lens.instantiations.enable = value
    });
    field!(&["qihe", "autoConfigureArgsFromManifest"], bool, |cfg, value| {
        cfg.qihe.auto_configure_args_from_manifest = value
    });
    field!(&["qihe", "command"], Option<String>, |cfg, value| cfg.qihe.command = value);
    field!(&["qihe", "compileArgs"], Vec<String>, |cfg, value| { cfg.qihe.compile_args = value });
    field!(&["qihe", "runArgs"], Vec<String>, |cfg, value| cfg.qihe.run_args = value);
    field!(&["references", "includeDeclaration"], bool, |cfg, value| {
        cfg.references.include_declaration = value
    });
    field!(&["scope", "visibility"], ScopeVisibility, |cfg, value| cfg.scope.visibility = value);
    field!(&["semantic", "tokens", "port", "clk", "rst", "enable"], bool, |cfg, value| {
        cfg.semantic.tokens.port.clk.rst.enable = value
    });
    field!(&["semantic", "tokens", "port", "input", "output", "enable"], bool, |cfg, value| {
        cfg.semantic.tokens.port.input.output.enable = value
    });
    field!(&["signature", "help", "params", "only"], bool, |cfg, value| {
        cfg.signature.help.params.only = value
    });
    field!(&["workspace", "auto", "reload"], bool, |cfg, value| {
        cfg.workspace.auto.reload = value
    });
}

impl UserConfig {
    pub fn from_json(
        json: serde_json::Value,
        error_sink: &mut Vec<(String, serde_json::Error)>,
    ) -> Self {
        if json.is_null() {
            return Self::default();
        }

        let mut config = Self::default();
        validate_user_config_shape(&json, &user_config_path_tree(), "", error_sink);
        if json.is_object() {
            apply_user_config_fields(&json, &mut config, error_sink);
        }
        config
    }
}
