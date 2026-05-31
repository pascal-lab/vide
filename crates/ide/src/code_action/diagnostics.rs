use utils::text_edit::TextRange;

#[derive(Debug, Clone, Default)]
pub struct CodeActionDiagnostics {
    pub items: Vec<CodeActionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeActionDiagnostic {
    pub source: Option<DiagnosticSource>,
    pub code: Option<DiagnosticCode>,
    pub name: Option<String>,
    pub option: Option<String>,
    pub range: Option<TextRange>,
    pub expected_token: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSource {
    Parse,
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticCode {
    pub subsystem: u16,
    pub code: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    MissingConnection,
    MissingParameter,
    ConvertOrderedPorts,
    ConvertOrderedParams,
    RemoveEmptyPortConnections,
    AddImplicitNamedPortParens,
    AddInstanceParens,
    InsertExpectedToken,
}

impl CodeActionDiagnostics {
    pub fn allows_repair(&self, repair: RepairKind) -> bool {
        self.items.iter().any(|diag| diag.allows_repair(repair))
    }
}

impl CodeActionDiagnostic {
    pub fn allows_repair(&self, repair: RepairKind) -> bool {
        match repair {
            RepairKind::MissingConnection => {
                self.source == Some(DiagnosticSource::Semantic)
                    && (matches!(
                        self.option.as_deref(),
                        Some("unconnected-port" | "unconnected-unnamed-port")
                    ) || self.code == Some(DiagnosticCode { subsystem: 2, code: 260 })
                        || self.code == Some(DiagnosticCode { subsystem: 2, code: 261 }))
            }
            RepairKind::MissingParameter => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("ParamHasNoValue")
            }
            RepairKind::ConvertOrderedPorts => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("MixingOrderedAndNamedPorts")
            }
            RepairKind::ConvertOrderedParams => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("MixingOrderedAndNamedParams")
            }
            RepairKind::RemoveEmptyPortConnections => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("MixingOrderedAndNamedPorts")
            }
            RepairKind::AddImplicitNamedPortParens => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("ImplicitNamedPortNotFound")
            }
            RepairKind::AddInstanceParens => {
                self.source == Some(DiagnosticSource::Semantic)
                    && self.name.as_deref() == Some("InstanceMissingParens")
            }
            RepairKind::InsertExpectedToken => {
                self.source == Some(DiagnosticSource::Parse)
                    && self.name.as_deref() == Some("ExpectedToken")
                    && self.expected_token.as_deref().is_some_and(safe_insertable_token)
            }
        }
    }
}

pub(crate) fn safe_insertable_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 32
        && token.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'_' | b'$'
                        | b'#'
                        | b'('
                        | b')'
                        | b'['
                        | b']'
                        | b'{'
                        | b'}'
                        | b';'
                        | b':'
                        | b','
                        | b'.'
                        | b'*'
                        | b'/'
                        | b'='
                        | b'<'
                        | b'>'
                        | b'+'
                        | b'-'
                        | b'!'
                        | b'?'
                        | b'@'
                        | b'`'
                )
        })
}
