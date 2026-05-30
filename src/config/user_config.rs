use base_db::diagnostics_config::{
    DiagnosticPhaseConfig, DiagnosticRule, DiagnosticRuleSeverity, DiagnosticSelector,
    DiagnosticSource, DiagnosticsConfig, SlangDiagnosticsConfig,
};
use ide::{
    code_lens::CodeLensConfig,
    document_highlight::DocumentHighlightConfig,
    formatting::{FmtConfig, FormatterProvider},
    hover::HoverConfig,
    inlay_hint::InlayHintConfig,
    references::ReferencesConfig,
    rename::RenameConfig,
    semantic_tokens::{SemaTokenConfig, SemaTokenPortConfig},
    signature_help::SignatureHelpConfig,
};
use serde::{Deserialize, Serialize};
use utils::{json::get_field, paths::Utf8PathBuf};

use super::Config;

const DEFAULT_QIHE_COMMAND: &str = "qihe";
const DEFAULT_QIHE_RUN_ARGS: &[&str] = &["-g", "std"];

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FilesWatcherDef {
    Client,
    Notify,
    Server,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScopeVisibility {
    Public,
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticsUserConfig {
    #[serde(default = "default_true")]
    pub(crate) enable: bool,
    #[serde(default)]
    pub(crate) update: DiagnosticsUpdateUserConfig,
    #[serde(default)]
    pub(crate) parse: DiagnosticsPhaseUserConfig,
    #[serde(default)]
    pub(crate) semantic: DiagnosticsPhaseUserConfig,
    #[serde(default)]
    pub(crate) slang: SlangDiagnosticsUserConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DiagnosticsUpdateUserConfig {
    OnType,
    #[default]
    OnSave,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticsPhaseUserConfig {
    #[serde(default = "default_true")]
    pub(crate) enable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct SlangDiagnosticsUserConfig {
    #[serde(default)]
    pub(crate) warnings: Vec<String>,
    #[serde(default)]
    pub(crate) rules: Vec<DiagnosticRuleUserConfig>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiagnosticRuleUserConfig {
    pub(crate) selector: String,
    pub(crate) severity: DiagnosticRuleSeverityUserConfig,
    #[serde(default)]
    pub(crate) force: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DiagnosticRuleSeverityUserConfig {
    Ignore,
    Info,
    Warning,
    Error,
    Fatal,
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

impl Default for DiagnosticsPhaseUserConfig {
    fn default() -> Self {
        Self { enable: true }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QiheConfig {
    pub(crate) command: String,
    pub(crate) auto_configure_args_from_manifest: bool,
    pub(crate) compile_args: Vec<String>,
    pub(crate) run_args: Vec<String>,
}

macro_rules! config_data {
    ($sv:vis struct $name:ident {
         $($(#[doc=$_:literal])*
         $field:ident : $ty:ty = $default:expr,)*
    }) => {
        #[allow(non_snake_case)]
        #[derive(Debug, Clone, PartialEq, Eq)]
        $sv struct $name {
            $($sv $field: $ty,)*
        }

        impl $name {
            $sv fn from_json(mut json: serde_json::Value, error_sink: &mut Vec<(String, serde_json::Error)>) -> $name {
                $name {
                    $( $field: get_field(&mut json, error_sink, stringify!($field), || $default), )*
                }
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self {
                    $($field: $default,)*
                }
            }
        }
    };
}

config_data! {
    pub(crate) struct UserConfig {
        /// These directories will be ignored. They are relative to the workspace
        /// root, and globs are not supported. You may also need to add the
        /// folders to Code's `files.watcherExclude`.
        files_excludeDirs: Vec<Utf8PathBuf> = vec![],
        /// Controls file watching.
        files_watcher: FilesWatcherDef = FilesWatcherDef::Client,
        /// Automatically refresh project info on toml changes
        workspace_auto_reload: bool = true,

        /// If true, symbols within a scope (except for ports) are private to other scopes.
        scope_visibility: ScopeVisibility = ScopeVisibility::Private,

        formatter_provider: FormatterProvider = FormatterProvider::Verible,
        formatter_path: Option<Utf8PathBuf> = None,
        formatting_on_enter: bool = true,
        formatting_in_comments: bool = true,
        formatting_indent_width: usize = 4,
        formatter_args: Vec<String> = vec![
            "--failsafe_success=false",
        ].into_iter().map(String::from).collect(),

        inlayHints_port_connection_enable: bool = true,
        inlayHints_parameter_assignment_enable: bool = true,
        inlayHints_end_structure_enable: bool = true,

        lens_instantiations_enable: bool = true,

        semantic_tokens_port_clk_rst_enable: bool = true,
        semantic_tokens_port_input_output_enable: bool = true,

        diagnostics: DiagnosticsUserConfig = DiagnosticsUserConfig::default(),

        signature_help_params_only: bool = false,

        qihe_command: String = DEFAULT_QIHE_COMMAND.to_string(),
        qihe_autoConfigureArgsFromManifest: bool = true,
        qihe_compileArgs: Vec<String> = vec![],
        qihe_runArgs: Vec<String> =
            DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_string()).collect(),
    }
}

impl UserConfig {
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
        let command = Some(self.qihe_command.trim())
            .filter(|cmd| !cmd.is_empty())
            .unwrap_or(DEFAULT_QIHE_COMMAND)
            .to_string();

        let run_args =
            Some(&self.qihe_runArgs).filter(|args| !args.is_empty()).cloned().unwrap_or_else(
                || DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_string()).collect(),
            );

        QiheConfig {
            command,
            auto_configure_args_from_manifest: self.qihe_autoConfigureArgsFromManifest,
            compile_args: self.qihe_compileArgs.clone(),
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
        let scope_visibility = self.user_config.scope_visibility.into();
        ReferencesConfig::new(scope_visibility, None)
    }

    pub(crate) fn document_highlight(&self) -> DocumentHighlightConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        DocumentHighlightConfig { scope_visibility }
    }

    pub(crate) fn rename(&self) -> RenameConfig {
        let scope_visibility = self.user_config.scope_visibility.into();
        RenameConfig::workspace(scope_visibility)
    }

    pub(crate) fn fmt(&self) -> FmtConfig {
        FmtConfig {
            provider: self.user_config.formatter_provider,
            executable: self.user_config.formatter_path.clone(),
            args: self.user_config.formatter_args.clone(),
            indent_width: self.user_config.formatting_indent_width,
            on_enter: self.user_config.formatting_on_enter,
            in_comments: self.user_config.formatting_in_comments,
        }
    }

    pub(crate) fn hover(&self) -> HoverConfig {
        HoverConfig { format: self.cli_hover_markdown_support() }
    }

    pub(crate) fn inlay_hint(&self) -> InlayHintConfig {
        InlayHintConfig {
            port_connection: self.user_config.inlayHints_port_connection_enable,
            parameter_assignment: self.user_config.inlayHints_parameter_assignment_enable,
            end_structure: self.user_config.inlayHints_end_structure_enable,
        }
    }

    pub(crate) fn code_lens(&self) -> CodeLensConfig {
        CodeLensConfig { instantiations: self.user_config.lens_instantiations_enable }
    }

    pub(crate) fn semantic_tokens(&self) -> SemaTokenConfig {
        SemaTokenConfig {
            port: SemaTokenPortConfig {
                clk_rst: self.user_config.semantic_tokens_port_clk_rst_enable,
                io: self.user_config.semantic_tokens_port_input_output_enable,
            },
        }
    }

    pub(crate) fn signature_help(&self) -> SignatureHelpConfig {
        SignatureHelpConfig { params_only: self.user_config.signature_help_params_only }
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
