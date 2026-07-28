mod def;
pub(crate) mod ffi;
mod syntax_diagnostic;

pub use def::*;
pub use syntax_diagnostic::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_diagnostic_metadata_matches_slang_definitions() {
        let expected_expression = DiagCode::EXPECTED_EXPRESSION.info().unwrap();
        assert_eq!(expected_expression.name, "ExpectedExpression");
        assert_eq!(expected_expression.subsystem, DiagSubsystem::General);
        assert_eq!(expected_expression.severity, DiagnosticSeverity::Error);
        assert_eq!(expected_expression.default_message, "expected expression");
        assert_eq!(expected_expression.option_name, None);

        let unknown_escape_code = DiagCode::UNKNOWN_ESCAPE_CODE.info().unwrap();
        assert_eq!(unknown_escape_code.subsystem, DiagSubsystem::Lexer);
        assert_eq!(unknown_escape_code.severity, DiagnosticSeverity::Warning);
        assert_eq!(unknown_escape_code.option_name, Some("unknown-escape-code"));

        let default_group = DIAGNOSTIC_GROUPS.iter().find(|group| group.name == "default").unwrap();
        assert!(default_group.diagnostics.contains(&DiagCode::UNKNOWN_ESCAPE_CODE));
    }
}
