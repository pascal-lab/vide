use std::sync::atomic::{AtomicU64, Ordering};

use syntax::DiagnosticSeverity;

static NEXT_CONFIG_REVISION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsConfig {
    pub revision: u64,
    pub enabled: bool,
    pub parse: DiagnosticPhaseConfig,
    pub semantic: DiagnosticPhaseConfig,
    pub slang: SlangDiagnosticsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticPhaseConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlangDiagnosticsConfig {
    pub warnings: Vec<String>,
    pub rules: Vec<DiagnosticRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticRule {
    pub selector: DiagnosticSelector,
    pub severity: DiagnosticRuleSeverity,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticSelector {
    Code { subsystem: u16, code: u16 },
    Option(String),
    Group(String),
    Source(DiagnosticSource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    Parse,
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticRuleSeverity {
    Ignore,
    Info,
    Warning,
    Error,
    Fatal,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            revision: NEXT_CONFIG_REVISION.fetch_add(1, Ordering::Relaxed),
            enabled: true,
            parse: DiagnosticPhaseConfig { enabled: true },
            semantic: DiagnosticPhaseConfig { enabled: true },
            slang: SlangDiagnosticsConfig { warnings: Vec::new(), rules: Vec::new() },
        }
    }
}

impl DiagnosticsConfig {
    pub fn with_fresh_revision(mut self) -> Self {
        self.revision = NEXT_CONFIG_REVISION.fetch_add(1, Ordering::Relaxed);
        self
    }

    pub fn apply_rules(
        &self,
        source: DiagnosticSource,
        mut diag: syntax::SyntaxDiagnostic,
    ) -> Option<syntax::SyntaxDiagnostic> {
        if !self.enabled {
            return None;
        }

        for rule in &self.slang.rules {
            if !rule.matches(source, &diag) {
                continue;
            }

            match rule.severity {
                DiagnosticRuleSeverity::Ignore => return None,
                DiagnosticRuleSeverity::Info => diag.severity = DiagnosticSeverity::Note,
                DiagnosticRuleSeverity::Warning => diag.severity = DiagnosticSeverity::Warning,
                DiagnosticRuleSeverity::Error => diag.severity = DiagnosticSeverity::Error,
                DiagnosticRuleSeverity::Fatal => diag.severity = DiagnosticSeverity::Fatal,
            }
        }

        (diag.severity != DiagnosticSeverity::Ignored).then_some(diag)
    }
}

impl DiagnosticRule {
    fn matches(&self, source: DiagnosticSource, diag: &syntax::SyntaxDiagnostic) -> bool {
        match &self.selector {
            DiagnosticSelector::Code { subsystem, code } => {
                diag.subsystem == *subsystem && diag.code == *code
            }
            DiagnosticSelector::Option(option) => diag.option_name.as_deref() == Some(option),
            DiagnosticSelector::Group(group) => diag.groups.iter().any(|it| it == group),
            DiagnosticSelector::Source(rule_source) => source == *rule_source,
        }
    }
}

#[cfg(test)]
mod tests {
    use syntax::{DiagnosticSeverity, SyntaxDiagnostic};

    use super::*;

    #[test]
    fn applies_source_rule() {
        let config = DiagnosticsConfig {
            slang: SlangDiagnosticsConfig {
                warnings: Vec::new(),
                rules: vec![DiagnosticRule {
                    selector: DiagnosticSelector::Source(DiagnosticSource::Parse),
                    severity: DiagnosticRuleSeverity::Ignore,
                    force: false,
                }],
            },
            ..DiagnosticsConfig::default()
        };

        let diag = SyntaxDiagnostic {
            code: 1,
            subsystem: 2,
            severity: DiagnosticSeverity::Error,
            message: "error".into(),
            name: "test".into(),
            option_name: None,
            groups: Vec::new(),
            primary_range: None,
            location: None,
            buffer_id: None,
            file_name: None,
        };

        assert!(config.apply_rules(DiagnosticSource::Parse, diag.clone()).is_none());
        assert!(config.apply_rules(DiagnosticSource::Semantic, diag).is_some());
    }
}
