use crate::diagnostics::{Diagnostic, DiagnosticSource};

/// The kind of diagnostic repair a code action satisfies. Matching lives in
/// the ide layer where the full diagnostic metadata (source, slang name,
/// code, option) is available; the LSP layer only renders what the engine
/// attaches to the action.
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

impl RepairKind {
    pub fn matches(self, diag: &Diagnostic) -> bool {
        match self {
            RepairKind::MissingConnection => {
                diag.source == DiagnosticSource::SlangSemantic
                    && (matches!(
                        diag.option_name.as_deref(),
                        Some("unconnected-port" | "unconnected-unnamed-port")
                    ) || (diag.subsystem == 2 && matches!(diag.code, 260 | 261)))
            }
            RepairKind::MissingParameter => {
                diag.source == DiagnosticSource::SlangSemantic && diag.name == "ParamHasNoValue"
            }
            RepairKind::ConvertOrderedPorts => {
                diag.source == DiagnosticSource::SlangSemantic
                    && diag.name == "MixingOrderedAndNamedPorts"
            }
            RepairKind::ConvertOrderedParams => {
                diag.source == DiagnosticSource::SlangSemantic
                    && diag.name == "MixingOrderedAndNamedParams"
            }
            RepairKind::RemoveEmptyPortConnections => {
                diag.source == DiagnosticSource::SlangSemantic
                    && diag.name == "MixingOrderedAndNamedPorts"
            }
            RepairKind::AddImplicitNamedPortParens => {
                diag.source == DiagnosticSource::SlangSemantic
                    && diag.name == "ImplicitNamedPortNotFound"
            }
            RepairKind::AddInstanceParens => {
                diag.source == DiagnosticSource::SlangSemantic
                    && diag.name == "InstanceMissingParens"
            }
            RepairKind::InsertExpectedToken => {
                diag.source == DiagnosticSource::SlangParse && diag.name == "ExpectedToken"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use syntax::DiagnosticSeverity;
    use utils::text_edit::TextSize;
    use vfs::FileId;

    use super::RepairKind;
    use crate::diagnostics::{Diagnostic, DiagnosticSource};

    fn diagnostic(name: &str, subsystem: u16, code: u16, option_name: Option<&str>) -> Diagnostic {
        Diagnostic {
            file_id: FileId::from_raw(0),
            code,
            subsystem,
            name: name.to_owned(),
            option_name: option_name.map(ToOwned::to_owned),
            groups: Vec::new(),
            source: DiagnosticSource::SlangSemantic,
            range: TextSize::from(0).into(),
            severity: DiagnosticSeverity::Error,
            message: String::new(),
            args: Vec::new(),
            message_key: None,
            message_args: Vec::new(),
            tags: Vec::new(),
        }
    }

    #[test]
    fn repair_kinds_match_their_diagnostic_metadata() {
        let cases = [
            (
                RepairKind::MissingConnection,
                diagnostic("UnconnectedNamedPort", 2, 260, Some("unconnected-port")),
            ),
            (RepairKind::MissingConnection, diagnostic("UnconnectedNamedPort", 2, 261, None)),
            (RepairKind::MissingParameter, diagnostic("ParamHasNoValue", 2, 29, None)),
            (RepairKind::ConvertOrderedPorts, diagnostic("MixingOrderedAndNamedPorts", 2, 0, None)),
            (
                RepairKind::ConvertOrderedParams,
                diagnostic("MixingOrderedAndNamedParams", 2, 0, None),
            ),
            (
                RepairKind::RemoveEmptyPortConnections,
                diagnostic("MixingOrderedAndNamedPorts", 2, 0, None),
            ),
            (
                RepairKind::AddImplicitNamedPortParens,
                diagnostic("ImplicitNamedPortNotFound", 2, 0, None),
            ),
            (RepairKind::AddInstanceParens, diagnostic("InstanceMissingParens", 2, 0, None)),
        ];

        for (repair, diag) in cases {
            assert!(repair.matches(&diag), "{repair:?} should match {:?}", diag.name);
        }

        let unrelated = diagnostic("MixingOrderedAndNamedPorts", 2, 0, None);
        assert!(!RepairKind::MissingParameter.matches(&unrelated));
    }

    #[test]
    fn repair_kind_matches_parse_expected_token() {
        let mut diag = diagnostic("ExpectedToken", 1, 116, None);
        diag.source = DiagnosticSource::SlangParse;
        diag.args = vec![";".to_owned()];

        assert!(RepairKind::InsertExpectedToken.matches(&diag));
    }
}
