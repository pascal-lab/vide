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
pub(crate) use user_config::*;

use super::Config;

#[cfg(windows)]
const DEFAULT_QIHE_COMMAND: &str = "qihe.bat";
#[cfg(not(windows))]
const DEFAULT_QIHE_COMMAND: &str = "qihe";
const DEFAULT_QIHE_RUN_ARGS: &[&str] = &["-g", "std"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QiheConfig {
    pub(crate) command: String,
    pub(crate) auto_configure_args_from_manifest: bool,
    pub(crate) compile_args: Vec<String>,
    pub(crate) run_args: Vec<String>,
}

pub(crate) fn diagnostics_config(user_config: &UserConfig) -> DiagnosticsConfig {
    DiagnosticsConfig {
        revision: 0,
        enabled: user_config.diagnostics.enable,
        parse: DiagnosticPhaseConfig { enabled: user_config.diagnostics.parse.enable },
        semantic: DiagnosticPhaseConfig { enabled: user_config.diagnostics.semantic.enable },
        slang: SlangDiagnosticsConfig {
            warnings: Some(user_config.diagnostics.slang.warnings.clone()),
            rules: user_config
                .diagnostics
                .slang
                .rules
                .iter()
                .filter_map(diagnostic_rule_config)
                .collect(),
        },
    }
}

fn qihe_config(user_config: &UserConfig) -> QiheConfig {
    let command = user_config
        .qihe
        .command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .unwrap_or(DEFAULT_QIHE_COMMAND)
        .to_owned();
    let run_args = if user_config.qihe.run_args.is_empty() {
        DEFAULT_QIHE_RUN_ARGS.iter().map(|arg| (*arg).to_owned()).collect()
    } else {
        user_config.qihe.run_args.clone()
    };

    QiheConfig {
        command,
        auto_configure_args_from_manifest: user_config.qihe.auto_configure_args_from_manifest,
        compile_args: user_config.qihe.compile_args.clone(),
        run_args,
    }
}

fn diagnostic_rule_config(rule: &DiagnosticRuleUserConfig) -> Option<DiagnosticRule> {
    Some(DiagnosticRule {
        selector: parse_selector(&rule.selector)?,
        severity: diagnostic_rule_severity(rule.severity),
        force: rule.force,
    })
}

fn diagnostic_rule_severity(value: DiagnosticRuleSeverityUserConfig) -> DiagnosticRuleSeverity {
    match value {
        DiagnosticRuleSeverityUserConfig::Ignore => DiagnosticRuleSeverity::Ignore,
        DiagnosticRuleSeverityUserConfig::Info => DiagnosticRuleSeverity::Info,
        DiagnosticRuleSeverityUserConfig::Warning => DiagnosticRuleSeverity::Warning,
        DiagnosticRuleSeverityUserConfig::Error => DiagnosticRuleSeverity::Error,
        DiagnosticRuleSeverityUserConfig::Fatal => DiagnosticRuleSeverity::Fatal,
    }
}

fn scope_visibility(value: ScopeVisibility) -> ide::ScopeVisibility {
    match value {
        ScopeVisibility::Public => ide::ScopeVisibility::Public,
        ScopeVisibility::Private => ide::ScopeVisibility::Private,
    }
}

fn formatter_provider(value: FormatterProviderUserConfig) -> FormatterProvider {
    match value {
        FormatterProviderUserConfig::Verible => FormatterProvider::Verible,
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
    pub(crate) fn references_include_declaration(&self) -> bool {
        self.user_config.references.include_declaration
    }

    pub(crate) fn references(&self) -> ReferencesConfig {
        ReferencesConfig::new(scope_visibility(self.user_config.scope.visibility), None)
    }

    pub(crate) fn document_highlight(&self) -> DocumentHighlightConfig {
        DocumentHighlightConfig {
            scope_visibility: scope_visibility(self.user_config.scope.visibility),
        }
    }

    pub(crate) fn rename(&self) -> RenameConfig {
        RenameConfig::workspace(scope_visibility(self.user_config.scope.visibility))
    }

    pub(crate) fn fmt(&self) -> FmtConfig {
        FmtConfig {
            provider: formatter_provider(self.user_config.formatter.provider),
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
            macro_argument: self.user_config.inlay_hints.r#macro.argument.enable,
            end_structure: self.user_config.inlay_hints.end.structure.enable,
            system_call: self.user_config.inlay_hints.system_call.call.enable,
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
        qihe_config(&self.user_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_keeps_width_warnings() {
        let user_config = UserConfig::default();
        assert_eq!(
            diagnostics_config(&user_config).slang.warnings,
            Some(vec![
                "width-expand".to_owned(),
                "width-trunc".to_owned(),
                "port-width-expand".to_owned(),
                "port-width-trunc".to_owned(),
            ])
        );
    }

    #[test]
    fn qihe_default_command_matches_current_platform() {
        let user_config = UserConfig::default();
        assert_eq!(qihe_config(&user_config).command, DEFAULT_QIHE_COMMAND);
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
        let user_config = UserConfig::from_json(json, &mut errors);
        assert!(errors.is_empty(), "{errors:?}");

        let config = diagnostics_config(&user_config);
        assert_eq!(user_config.diagnostics.update, DiagnosticsUpdateUserConfig::OnType);
        assert!(config.parse.enabled);
        assert!(!config.semantic.enabled);
        assert_eq!(config.slang.warnings, Some(vec!["default".to_owned(), "no-unused".to_owned()]));
        assert_eq!(config.slang.rules.len(), 2);
    }

    #[test]
    fn parses_system_call_inlay_hint_config() {
        let mut errors = vec![];
        let user_config = UserConfig::from_json(
            serde_json::json!({
                "inlayHints": { "systemCall": { "call": { "enable": false } } }
            }),
            &mut errors,
        );

        assert!(errors.is_empty(), "{errors:?}");
        assert!(!user_config.inlay_hints.system_call.call.enable);
    }
}
