use base_db::diagnostics_config::{
    DiagnosticPhaseConfig, DiagnosticRule, DiagnosticRuleSeverity, DiagnosticSelector,
    DiagnosticSource, DiagnosticsConfig, SlangDiagnosticsConfig,
};
use ide::{
    code_lens::CodeLensConfig,
    document_highlight::DocumentHighlightConfig,
    formatting::FmtConfig,
    hover::HoverConfig,
    inlay_hint::InlayHintConfig,
    references::ReferencesConfig,
    rename::RenameConfig,
    semantic_tokens::{SemaTokenConfig, SemaTokenPortConfig},
    signature_help::SignatureHelpConfig,
};
use serde::{Deserialize, Serialize};
use utils::paths::Utf8PathBuf;

use super::Config;

const DEFAULT_QIHE_COMMAND: &str = "qihe";
const DEFAULT_QIHE_RUN_ARGS: &[&str] = &["-g", "std"];
#[cfg(feature = "user-config-schema")]
const USER_CONFIG_SCHEMA_URL: &str =
    "https://vide.pascal-lab.net/schemas/v1/user-config.schema.json";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum FilesWatcherDef {
    #[default]
    Client,
    Notify,
    Server,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScopeVisibility {
    Public,
    #[default]
    Private,
}

impl From<ScopeVisibility> for ide::ScopeVisibility {
    fn from(val: ScopeVisibility) -> Self {
        match val {
            ScopeVisibility::Public => ide::ScopeVisibility::Public,
            ScopeVisibility::Private => ide::ScopeVisibility::Private,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub(crate) enum FormatterProviderUserConfig {
    #[default]
    Verible,
}

impl From<FormatterProviderUserConfig> for ide::formatting::FormatterProvider {
    fn from(provider: FormatterProviderUserConfig) -> Self {
        match provider {
            FormatterProviderUserConfig::Verible => ide::formatting::FormatterProvider::Verible,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub(crate) enum DiagnosticsUpdateUserConfig {
    OnType,
    #[default]
    OnSave,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub(crate) enum DiagnosticRuleSeverityUserConfig {
    Ignore,
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QiheConfig {
    pub(crate) command: String,
    pub(crate) auto_configure_args_from_manifest: bool,
    pub(crate) compile_args: Vec<String>,
    pub(crate) run_args: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(
    feature = "user-config-schema",
    schemars(
        title = "Vide language server user configuration",
        description = "Initialization options and dynamic configuration accepted by the Vide language server. These options are useful for editors that configure LSP servers directly, such as Neovim and Emacs.",
        deny_unknown_fields
    )
)]
pub(crate) struct UserConfig {
    pub(crate) files: FilesUserConfig,
    pub(crate) workspace: WorkspaceUserConfig,
    pub(crate) scope: ScopeUserConfig,
    pub(crate) references: ReferencesUserConfig,
    pub(crate) formatter: FormatterUserConfig,
    pub(crate) formatting: FormattingUserConfig,
    #[serde(rename = "inlayHints")]
    pub(crate) inlay_hints: InlayHintsUserConfig,
    pub(crate) lens: LensUserConfig,
    pub(crate) semantic: SemanticUserConfig,
    pub(crate) diagnostics: DiagnosticsUserConfig,
    pub(crate) signature: SignatureUserConfig,
    pub(crate) qihe: QiheUserConfig,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            files: FilesUserConfig::default(),
            workspace: WorkspaceUserConfig::default(),
            scope: ScopeUserConfig::default(),
            references: ReferencesUserConfig::default(),
            formatter: FormatterUserConfig::default(),
            formatting: FormattingUserConfig::default(),
            inlay_hints: InlayHintsUserConfig::default(),
            lens: LensUserConfig::default(),
            semantic: SemanticUserConfig::default(),
            diagnostics: DiagnosticsUserConfig::default(),
            signature: SignatureUserConfig::default(),
            qihe: QiheUserConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct FilesUserConfig {
    /// These directories will be ignored. They are relative to the workspace
    /// root, and globs are not supported. You may also need to add the folders
    /// to VS Code's `files.watcherExclude`.
    #[serde(rename = "excludeDirs")]
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Workspace-relative directories ignored by Vide. Globs are not supported.",
            with = "Vec::<String>",
            default = "empty_string_vec"
        )
    )]
    pub(crate) exclude_dirs: Vec<Utf8PathBuf>,
    /// Controls file watching.
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Controls how Vide watches project files.",
            default = "FilesWatcherDef::default"
        )
    )]
    pub(crate) watcher: FilesWatcherDef,
}

impl Default for FilesUserConfig {
    fn default() -> Self {
        Self { exclude_dirs: Vec::new(), watcher: FilesWatcherDef::Client }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct WorkspaceUserConfig {
    pub(crate) auto: WorkspaceAutoUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct WorkspaceAutoUserConfig {
    /// Automatically refresh project info on toml changes.
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Automatically refresh project information when project manifests change.",
            default = "default_true"
        )
    )]
    pub(crate) reload: bool,
}

impl Default for WorkspaceAutoUserConfig {
    fn default() -> Self {
        Self { reload: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct ScopeUserConfig {
    /// If true, symbols within a scope, except for ports, are private to other
    /// scopes.
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Controls whether symbols inside scopes, except ports, are visible outside those scopes.",
            default = "ScopeVisibility::default"
        )
    )]
    pub(crate) visibility: ScopeVisibility,
}

impl Default for ScopeUserConfig {
    fn default() -> Self {
        Self { visibility: ScopeVisibility::Private }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct ReferencesUserConfig {
    #[serde(rename = "includeDeclaration")]
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Include declarations when finding references.",
            default = "default_true"
        )
    )]
    pub(crate) include_declaration: bool,
}

impl Default for ReferencesUserConfig {
    fn default() -> Self {
        Self { include_declaration: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct FormatterUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Formatter backend used by Vide.",
            default = "FormatterProviderUserConfig::default"
        )
    )]
    pub(crate) provider: FormatterProviderUserConfig,
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Path to verible-verilog-format when formatter.provider is verible. Use null to find it on PATH.",
            with = "Option::<String>"
        )
    )]
    pub(crate) path: Option<Utf8PathBuf>,
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Arguments passed to verible-verilog-format when formatter.provider is verible.",
            default = "default_formatter_args"
        )
    )]
    pub(crate) args: Vec<String>,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct FormattingUserConfig {
    pub(crate) on: FormattingOnUserConfig,
    pub(crate) r#in: FormattingInUserConfig,
    pub(crate) indent: FormattingIndentUserConfig,
}

impl Default for FormattingUserConfig {
    fn default() -> Self {
        Self {
            on: FormattingOnUserConfig::default(),
            r#in: FormattingInUserConfig::default(),
            indent: FormattingIndentUserConfig::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct FormattingOnUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Enable formatting behavior when pressing Enter.",
            default = "default_true"
        )
    )]
    pub(crate) enter: bool,
}

impl Default for FormattingOnUserConfig {
    fn default() -> Self {
        Self { enter: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct FormattingInUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(description = "Enable formatting inside comments.", default = "default_true")
    )]
    pub(crate) comments: bool,
}

impl Default for FormattingInUserConfig {
    fn default() -> Self {
        Self { comments: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct FormattingIndentUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Fallback indentation width used when editor formatting options are unavailable.",
            default = "default_indent_width",
            range(min = 0)
        )
    )]
    pub(crate) width: usize,
}

impl Default for FormattingIndentUserConfig {
    fn default() -> Self {
        Self { width: 4 }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct InlayHintsUserConfig {
    pub(crate) port: InlayHintsPortUserConfig,
    pub(crate) parameter: InlayHintsParameterUserConfig,
    pub(crate) end: InlayHintsEndUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct InlayHintsPortUserConfig {
    pub(crate) connection: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct InlayHintsParameterUserConfig {
    pub(crate) assignment: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct InlayHintsEndUserConfig {
    pub(crate) structure: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct LensUserConfig {
    pub(crate) instantiations: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SemanticUserConfig {
    pub(crate) tokens: SemanticTokensUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SemanticTokensUserConfig {
    pub(crate) port: SemanticTokensPortUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SemanticTokensPortUserConfig {
    pub(crate) clk: SemanticTokensClockUserConfig,
    pub(crate) input: SemanticTokensInputUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SemanticTokensClockUserConfig {
    pub(crate) rst: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SemanticTokensInputUserConfig {
    pub(crate) output: EnableUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct DiagnosticsUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(description = "Enable diagnostics.", default = "default_true")
    )]
    pub(crate) enable: bool,
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Controls when diagnostics are refreshed.",
            default = "DiagnosticsUpdateUserConfig::default"
        )
    )]
    pub(crate) update: DiagnosticsUpdateUserConfig,
    pub(crate) parse: DiagnosticsPhaseUserConfig,
    pub(crate) semantic: DiagnosticsPhaseUserConfig,
    pub(crate) slang: SlangDiagnosticsUserConfig,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct DiagnosticsPhaseUserConfig {
    #[cfg_attr(feature = "user-config-schema", schemars(default = "default_true"))]
    pub(crate) enable: bool,
}

impl Default for DiagnosticsPhaseUserConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SlangDiagnosticsUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Additional slang warning groups or aliases to enable.",
            default = "empty_string_vec"
        )
    )]
    pub(crate) warnings: Vec<String>,
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(description = "Per-diagnostic severity overrides.")
    )]
    pub(crate) rules: Vec<DiagnosticRuleUserConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct DiagnosticRuleUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(regex(
            pattern = "^(code:[0-9]+:[0-9]+|option:.+|group:.+|source:(parse|semantic))$"
        ))
    )]
    pub(crate) selector: String,
    pub(crate) severity: DiagnosticRuleSeverityUserConfig,
    #[serde(default)]
    pub(crate) force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SignatureUserConfig {
    pub(crate) help: SignatureHelpUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SignatureHelpUserConfig {
    pub(crate) params: SignatureHelpParamsUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct SignatureHelpParamsUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(description = "Only show parameter signature help.")
    )]
    pub(crate) only: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct QiheUserConfig {
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(description = "Command used to invoke Qihe.", default = "default_qihe_command")
    )]
    pub(crate) command: String,
    #[serde(rename = "autoConfigureArgsFromManifest")]
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Automatically add Qihe compile mode and forwarded slang options from the Vide project manifest.",
            default = "default_true"
        )
    )]
    pub(crate) auto_configure_args_from_manifest: bool,
    #[serde(rename = "compileArgs")]
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Arguments inserted after qihe compile.",
            default = "empty_string_vec"
        )
    )]
    pub(crate) compile_args: Vec<String>,
    #[serde(rename = "runArgs")]
    #[cfg_attr(
        feature = "user-config-schema",
        schemars(
            description = "Arguments inserted after qihe run.",
            default = "default_qihe_run_args"
        )
    )]
    pub(crate) run_args: Vec<String>,
}

impl Default for QiheUserConfig {
    fn default() -> Self {
        Self {
            command: DEFAULT_QIHE_COMMAND.to_owned(),
            auto_configure_args_from_manifest: true,
            compile_args: Vec::new(),
            run_args: DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_owned()).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "user-config-schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
#[cfg_attr(feature = "user-config-schema", schemars(deny_unknown_fields))]
pub(crate) struct EnableUserConfig {
    #[cfg_attr(feature = "user-config-schema", schemars(default = "default_true"))]
    pub(crate) enable: bool,
}

impl Default for EnableUserConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

#[cfg(feature = "user-config-schema")]
fn default_true() -> bool {
    true
}

#[cfg(feature = "user-config-schema")]
fn empty_string_vec() -> Vec<String> {
    Vec::new()
}

#[cfg(feature = "user-config-schema")]
fn default_formatter_args() -> Vec<String> {
    vec!["--failsafe_success=false".to_owned()]
}

#[cfg(feature = "user-config-schema")]
fn default_indent_width() -> usize {
    4
}

#[cfg(feature = "user-config-schema")]
fn default_qihe_command() -> String {
    DEFAULT_QIHE_COMMAND.to_owned()
}

#[cfg(feature = "user-config-schema")]
fn default_qihe_run_args() -> Vec<String> {
    DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_owned()).collect()
}

#[cfg(feature = "user-config-schema")]
pub fn generated_user_config_schema() -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(UserConfig))
        .expect("user config schema should serialize");
    if let Some(root) = schema.as_object_mut() {
        root.insert("$id".to_owned(), serde_json::json!(USER_CONFIG_SCHEMA_URL));
        root.insert("x-vide-config-kind".to_owned(), serde_json::json!("user"));
    }
    schema
}

#[cfg(feature = "user-config-schema")]
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

#[cfg(feature = "user-config-schema")]
#[derive(Clone, Copy)]
enum ConfigSettingDefault {
    Bool(bool),
    String(&'static str),
    Null,
    StringArray(&'static [&'static str]),
    Usize(usize),
}

#[cfg(feature = "user-config-schema")]
#[derive(Clone, Copy)]
enum ConfigSettingSchema {
    Boolean,
    String,
    StringOrNull,
    StringArray,
    Integer { minimum: usize },
    Enum { values: &'static [&'static str] },
    DiagnosticRules,
}

#[cfg(feature = "user-config-schema")]
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
            ConfigSettingSchema::String => {
                property.insert("type".to_owned(), serde_json::json!("string"));
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

#[cfg(feature = "user-config-schema")]
const FILES_WATCHER_ENUM_DESCRIPTIONS: &[(&str, &str)] = &[
    ("client", "configuration.files.watcher.enum.client"),
    ("notify", "configuration.files.watcher.enum.notify"),
    ("server", "configuration.files.watcher.enum.server"),
];

#[cfg(feature = "user-config-schema")]
const FORMATTER_PROVIDER_ENUM_DESCRIPTIONS: &[(&str, &str)] =
    &[("verible", "configuration.formatter.provider.enum.verible")];

#[cfg(feature = "user-config-schema")]
const USER_CONFIG_SETTINGS: &[ConfigSettingMeta] = &[
    ConfigSettingMeta {
        path: &["qihe", "command"],
        vscode_key: "vide.qihe.command",
        docs_group: "Qihe",
        description_key: "configuration.qihe.command.description",
        markdown_description_key: None,
        enum_descriptions: &[],
        exposed_in_vscode: true,
        default: ConfigSettingDefault::String(DEFAULT_QIHE_COMMAND),
        schema: ConfigSettingSchema::String,
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
        default: ConfigSettingDefault::StringArray(&[]),
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

#[cfg(feature = "user-config-schema")]
pub fn generated_vscode_package_properties() -> serde_json::Map<String, serde_json::Value> {
    USER_CONFIG_SETTINGS
        .iter()
        .filter(|setting| setting.exposed_in_vscode)
        .map(|setting| (setting.vscode_key.to_owned(), setting.package_property()))
        .collect()
}

#[cfg(feature = "user-config-schema")]
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

impl UserConfig {
    pub(crate) fn from_json(
        json: serde_json::Value,
        error_sink: &mut Vec<(String, serde_json::Error)>,
    ) -> Self {
        if json.is_null() {
            return Self::default();
        }

        serde_json::from_value(json).unwrap_or_else(|err| {
            error_sink.push(("/".to_owned(), err));
            Self::default()
        })
    }

    pub(crate) fn diagnostics_config(&self) -> DiagnosticsConfig {
        DiagnosticsConfig {
            revision: 0,
            enabled: self.diagnostics.enable,
            parse: DiagnosticPhaseConfig { enabled: self.diagnostics.parse.enable },
            semantic: DiagnosticPhaseConfig { enabled: self.diagnostics.semantic.enable },
            slang: SlangDiagnosticsConfig {
                warnings: self.diagnostics.slang.warnings.clone(),
                rules: self
                    .diagnostics
                    .slang
                    .rules
                    .iter()
                    .filter_map(DiagnosticRuleUserConfig::to_config)
                    .collect(),
            },
        }
    }

    pub(crate) fn qihe(&self) -> QiheConfig {
        let command = Some(self.qihe.command.trim())
            .filter(|cmd| !cmd.is_empty())
            .unwrap_or(DEFAULT_QIHE_COMMAND)
            .to_string();

        let run_args =
            Some(&self.qihe.run_args).filter(|args| !args.is_empty()).cloned().unwrap_or_else(
                || DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_string()).collect(),
            );

        QiheConfig {
            command,
            auto_configure_args_from_manifest: self.qihe.auto_configure_args_from_manifest,
            compile_args: self.qihe.compile_args.clone(),
            run_args,
        }
    }
}

impl DiagnosticRuleUserConfig {
    fn to_config(&self) -> Option<DiagnosticRule> {
        Some(DiagnosticRule {
            selector: parse_selector(&self.selector)?,
            severity: self.severity.into(),
            force: self.force,
        })
    }
}

impl From<DiagnosticRuleSeverityUserConfig> for DiagnosticRuleSeverity {
    fn from(value: DiagnosticRuleSeverityUserConfig) -> Self {
        match value {
            DiagnosticRuleSeverityUserConfig::Ignore => DiagnosticRuleSeverity::Ignore,
            DiagnosticRuleSeverityUserConfig::Info => DiagnosticRuleSeverity::Info,
            DiagnosticRuleSeverityUserConfig::Warning => DiagnosticRuleSeverity::Warning,
            DiagnosticRuleSeverityUserConfig::Error => DiagnosticRuleSeverity::Error,
            DiagnosticRuleSeverityUserConfig::Fatal => DiagnosticRuleSeverity::Fatal,
        }
    }
}

fn parse_selector(selector: &str) -> Option<DiagnosticSelector> {
    let (kind, value) = selector.split_once(':')?;
    match kind {
        "code" => {
            let (subsystem, code) = value.split_once(':')?;
            Some(DiagnosticSelector::Code {
                subsystem: subsystem.parse().ok()?,
                code: code.parse().ok()?,
            })
        }
        "option" => Some(DiagnosticSelector::Option(value.to_owned())),
        "group" => Some(DiagnosticSelector::Group(value.to_owned())),
        "source" => match value {
            "parse" => Some(DiagnosticSelector::Source(DiagnosticSource::Parse)),
            "semantic" => Some(DiagnosticSelector::Source(DiagnosticSource::Semantic)),
            _ => None,
        },
        _ => None,
    }
}

impl Config {
    pub(crate) fn references(&self) -> ReferencesConfig {
        let scope_visibility = self.user_config.scope.visibility.into();
        ReferencesConfig::new(scope_visibility, None)
    }

    pub(crate) fn document_highlight(&self) -> DocumentHighlightConfig {
        let scope_visibility = self.user_config.scope.visibility.into();
        DocumentHighlightConfig { scope_visibility }
    }

    pub(crate) fn rename(&self) -> RenameConfig {
        let scope_visibility = self.user_config.scope.visibility.into();
        RenameConfig::workspace(scope_visibility)
    }

    pub(crate) fn fmt(&self) -> FmtConfig {
        FmtConfig {
            provider: self.user_config.formatter.provider.into(),
            executable: self.user_config.formatter.path.clone(),
            args: self.user_config.formatter.args.clone(),
            indent_width: self.user_config.formatting.indent.width,
            on_enter: self.user_config.formatting.on.enter,
            in_comments: self.user_config.formatting.r#in.comments,
        }
    }

    pub(crate) fn hover(&self) -> HoverConfig {
        HoverConfig { format: self.cli_hover_markdown_support() }
    }

    pub(crate) fn inlay_hint(&self) -> InlayHintConfig {
        InlayHintConfig {
            port_connection: self.user_config.inlay_hints.port.connection.enable,
            parameter_assignment: self.user_config.inlay_hints.parameter.assignment.enable,
            end_structure: self.user_config.inlay_hints.end.structure.enable,
        }
    }

    pub(crate) fn code_lens(&self) -> CodeLensConfig {
        CodeLensConfig { instantiations: self.user_config.lens.instantiations.enable }
    }

    pub(crate) fn semantic_tokens(&self) -> SemaTokenConfig {
        SemaTokenConfig {
            port: SemaTokenPortConfig {
                clk_rst: self.user_config.semantic.tokens.port.clk.rst.enable,
                io: self.user_config.semantic.tokens.port.input.output.enable,
            },
        }
    }

    pub(crate) fn signature_help(&self) -> SignatureHelpConfig {
        SignatureHelpConfig { params_only: self.user_config.signature.help.params.only }
    }

    pub(crate) fn qihe(&self) -> QiheConfig {
        self.user_config.qihe()
    }
}

#[test]
fn check_default() {
    let json = serde_json::Value::Null;
    let mut errors = vec![];
    let user_cfg = UserConfig::from_json(json, &mut errors);
    assert_eq!(user_cfg, UserConfig::default());
}

#[test]
fn parses_nested_diagnostics_config() {
    let json = serde_json::json!({
        "diagnostics": {
            "update": "onType",
            "semantic": { "enable": false },
            "slang": {
                "warnings": ["default", "no-unused"],
                "rules": [
                    { "selector": "source:parse", "severity": "ignore" },
                    { "selector": "code:1:2", "severity": "error", "force": true }
                ]
            }
        }
    });
    let mut errors = vec![];
    let user_cfg = UserConfig::from_json(json, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");

    let config = user_cfg.diagnostics_config();
    assert_eq!(user_cfg.diagnostics.update, DiagnosticsUpdateUserConfig::OnType);
    assert!(config.parse.enabled);
    assert!(!config.semantic.enabled);
    assert_eq!(config.slang.warnings, ["default", "no-unused"]);
    assert_eq!(config.slang.rules.len(), 2);
}

#[test]
fn parses_qihe_manifest_arg_configuration() {
    let json = serde_json::json!({
        "qihe": {
            "autoConfigureArgsFromManifest": false,
            "compileArgs": ["--mode", "sv2017", "--", "-I", "vendor/include"],
            "runArgs": ["-g", "std", "--foo"],
        }
    });
    let mut errors = vec![];
    let user_cfg = UserConfig::from_json(json, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");

    let qihe = user_cfg.qihe();
    assert!(!qihe.auto_configure_args_from_manifest);
    assert_eq!(qihe.compile_args, ["--mode", "sv2017", "--", "-I", "vendor/include"]);
    assert_eq!(qihe.run_args, ["-g", "std", "--foo"]);
}
