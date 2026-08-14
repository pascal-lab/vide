use syntax::diagnostics::DiagCode;

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
    /// Returns whether `diag` is the diagnostic this repair satisfies.
    ///
    /// The matchers use generated Slang diagnostic codes. Their symbolic
    /// names are regenerated from the pinned Slang descriptors, so a Slang
    /// upgrade cannot silently leave a quick fix coupled to a stale display
    /// string or v7 numeric id.
    pub fn matches(self, diag: &Diagnostic) -> bool {
        match self {
            RepairKind::MissingConnection => {
                diag.source == DiagnosticSource::SlangSemantic
                    && matches!(
                        DiagCode::from_raw(diag.subsystem, diag.code),
                        DiagCode::UNCONNECTED_IN_OUT_PORT
                            | DiagCode::UNCONNECTED_INPUT_PORT
                            | DiagCode::UNCONNECTED_OUTPUT_PORT
                    )
            }
            RepairKind::MissingParameter => {
                diag.source == DiagnosticSource::SlangSemantic
                    && DiagCode::from_raw(diag.subsystem, diag.code) == DiagCode::PARAM_HAS_NO_VALUE
            }
            RepairKind::ConvertOrderedPorts => {
                diag.source == DiagnosticSource::SlangSemantic
                    && DiagCode::from_raw(diag.subsystem, diag.code)
                        == DiagCode::MIXING_ORDERED_AND_NAMED_PORTS
            }
            RepairKind::ConvertOrderedParams => {
                diag.source == DiagnosticSource::SlangSemantic
                    && DiagCode::from_raw(diag.subsystem, diag.code)
                        == DiagCode::MIXING_ORDERED_AND_NAMED_PARAMS
            }
            RepairKind::RemoveEmptyPortConnections => {
                diag.source == DiagnosticSource::SlangSemantic
                    && DiagCode::from_raw(diag.subsystem, diag.code)
                        == DiagCode::MIXING_ORDERED_AND_NAMED_PORTS
            }
            RepairKind::AddImplicitNamedPortParens => {
                diag.source == DiagnosticSource::SlangSemantic
                    && DiagCode::from_raw(diag.subsystem, diag.code)
                        == DiagCode::IMPLICIT_NAMED_PORT_NOT_FOUND
            }
            RepairKind::AddInstanceParens => {
                diag.source == DiagnosticSource::SlangSemantic
                    && DiagCode::from_raw(diag.subsystem, diag.code)
                        == DiagCode::INSTANCE_MISSING_PARENS
            }
            RepairKind::InsertExpectedToken => {
                diag.source == DiagnosticSource::SlangParse
                    && DiagCode::from_raw(diag.subsystem, diag.code) == DiagCode::EXPECTED_TOKEN
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use syntax::diagnostics::{DiagCode, DiagnosticSeverity};
    use utils::text_edit::{TextRange, TextSize};
    use vfs::FileId;

    use super::RepairKind;
    use crate::diagnostics::{Diagnostic, DiagnosticSource};

    fn diagnostic(code: DiagCode, option_name: Option<&str>) -> Diagnostic {
        Diagnostic {
            file_id: FileId::from_raw(0),
            code: code.code_raw(),
            subsystem: code.subsystem_raw(),
            name: code.info().expect("test diagnostic should have metadata").name.to_owned(),
            option_name: option_name.map(ToOwned::to_owned),
            groups: Vec::new(),
            source: DiagnosticSource::SlangSemantic,
            range: TextRange::empty(TextSize::from(0)),
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
                diagnostic(DiagCode::UNCONNECTED_INPUT_PORT, Some("unconnected-input-port")),
            ),
            (RepairKind::MissingConnection, diagnostic(DiagCode::UNCONNECTED_OUTPUT_PORT, None)),
            (RepairKind::MissingParameter, diagnostic(DiagCode::PARAM_HAS_NO_VALUE, None)),
            (
                RepairKind::ConvertOrderedPorts,
                diagnostic(DiagCode::MIXING_ORDERED_AND_NAMED_PORTS, None),
            ),
            (
                RepairKind::ConvertOrderedParams,
                diagnostic(DiagCode::MIXING_ORDERED_AND_NAMED_PARAMS, None),
            ),
            (
                RepairKind::RemoveEmptyPortConnections,
                diagnostic(DiagCode::MIXING_ORDERED_AND_NAMED_PORTS, None),
            ),
            (
                RepairKind::AddImplicitNamedPortParens,
                diagnostic(DiagCode::IMPLICIT_NAMED_PORT_NOT_FOUND, None),
            ),
            (RepairKind::AddInstanceParens, diagnostic(DiagCode::INSTANCE_MISSING_PARENS, None)),
        ];

        for (repair, diag) in cases {
            assert!(repair.matches(&diag), "{repair:?} should match {:?}", diag.name);
        }

        let unrelated = diagnostic(DiagCode::MIXING_ORDERED_AND_NAMED_PORTS, None);
        assert!(!RepairKind::MissingParameter.matches(&unrelated));
    }

    #[test]
    fn repair_kind_matches_parse_expected_token() {
        let mut diag = diagnostic(DiagCode::EXPECTED_TOKEN, None);
        diag.source = DiagnosticSource::SlangParse;
        diag.args = vec![";".to_owned()];

        assert!(RepairKind::InsertExpectedToken.matches(&diag));
    }
}
