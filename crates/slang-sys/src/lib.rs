pub mod diagnostic;
pub mod ffi;
pub mod syntax;
pub mod token;

pub fn parse_root_kind(text: &str) -> syntax::SyntaxKind {
    syntax::SyntaxKind::from_raw(ffi::parse_root_kind(text))
}

#[cfg(test)]
mod tests {
    use crate::{diagnostic, syntax};
    #[test]
    fn rust_calls_upstream_slang_parser() {
        let test_verilog_code = r#"
module demo(
    input wire a,
    output wire b
);
begin
    assign b = a;
end
endmodule
        "#;
        let root_kind = crate::parse_root_kind(test_verilog_code);

        assert_eq!(root_kind, syntax::SyntaxKind::MODULE_DECLARATION);
    }

    #[test]
    fn generated_diagnostic_metadata_matches_slang_definitions() {
        let expected_expression = diagnostic::DiagCode::EXPECTED_EXPRESSION.info().unwrap();
        assert_eq!(expected_expression.name, "ExpectedExpression");
        assert_eq!(expected_expression.subsystem, diagnostic::DiagSubsystem::General);
        assert_eq!(expected_expression.severity, diagnostic::DiagnosticSeverity::Error);
        assert_eq!(expected_expression.default_message, "expected expression");
        assert_eq!(expected_expression.option_name, None);

        let unknown_escape_code = diagnostic::DiagCode::UNKNOWN_ESCAPE_CODE.info().unwrap();
        assert_eq!(unknown_escape_code.subsystem, diagnostic::DiagSubsystem::Lexer);
        assert_eq!(unknown_escape_code.severity, diagnostic::DiagnosticSeverity::Warning);
        assert_eq!(unknown_escape_code.option_name, Some("unknown-escape-code"));

        let default_group =
            diagnostic::DIAGNOSTIC_GROUPS.iter().find(|group| group.name == "default").unwrap();
        assert!(default_group.diagnostics.contains(&diagnostic::DiagCode::UNKNOWN_ESCAPE_CODE));
    }
}
