use std::ops::Range;

use super::{DIAGNOSTIC_GROUPS, DiagnosticSeverity, ffi};
use crate::{diagnostic::DiagCode, syntax::SyntaxKind, token::TokenKind};

/// Diagnostic emitted by Slang.
/// It's converted from diagnostic structures in cpp into Rust-owned data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub code: u16,
    pub subsystem: u16,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub args: Vec<String>,
    pub name: String,
    pub option_name: Option<String>,
    pub groups: Vec<String>,
    pub primary_range: Option<Range<usize>>,
    pub location: Option<usize>,
    pub buffer_id: Option<u32>,
    pub file_name: Option<String>,
}

/// Parser completion candidate reported by Slang at a cursor location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserExpectedSyntax {
    pub code: u16,
    pub subsystem: u16,
    pub name: String,
    pub token_kind: TokenKind,
    pub keyword_context: Option<SyntaxKeywordContext>,
    pub location: Option<usize>,
    pub end: Option<usize>,
}

/// Token or directive lexed at a source offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexedTokenAtOffset {
    pub replacement: Range<usize>,
    pub prefix: String,
    pub token_kind: TokenKind,
    pub directive_kind: Option<SyntaxKind>,
}

/// Context in which a SystemVerilog keyword is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SyntaxKeywordContext {
    CompilationUnitMember,
    LibraryMapMember,
    ModuleHeaderItem,
    ModuleMember,
    GenerateMember,
    SpecifyItem,
    ConfigHeaderItem,
    ConfigRule,
    BlockItem,
    Statement,
    ParameterPortListItem,
    AnsiPortItem,
    FunctionPortItem,
    GateType,
}

impl SyntaxDiagnostic {
    pub(crate) fn from_raw(raw: ffi::RawSyntaxDiagnostic) -> Self {
        let code = DiagCode::from_raw(raw.subsystem, raw.code);
        let severity = DiagnosticSeverity::from_raw(raw.severity).unwrap_or_else(|| {
            tracing::warn!(
                raw = raw.severity,
                code = raw.code,
                subsystem = raw.subsystem,
                "Slang returned an unknown diagnostic severity; treating it as fatal"
            );
            DiagnosticSeverity::Fatal
        });
        Self {
            code: raw.code,
            subsystem: raw.subsystem,
            severity,
            message: raw.message,
            args: raw.args,
            name: raw.name,
            option_name: (!raw.option_name.is_empty()).then_some(raw.option_name),
            groups: find_diagnostic_groups(code),
            primary_range: raw
                .has_primary_range
                .then_some(raw.primary_range_start..raw.primary_range_end),
            location: raw.has_location.then_some(raw.location),
            buffer_id: raw.has_buffer_id.then_some(raw.buffer_id),
            file_name: (!raw.file_name.is_empty()).then_some(raw.file_name),
        }
    }
}

fn find_diagnostic_groups(code: DiagCode) -> Vec<String> {
    DIAGNOSTIC_GROUPS
        .iter()
        .filter(|group| group.diagnostics.contains(&code))
        .map(|group| group.name.to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxTree;

    #[test]
    fn raw_diagnostic_groups_are_derived_from_generated_metadata() {
        let code = DiagCode::UNKNOWN_ESCAPE_CODE;
        let diagnostic = SyntaxDiagnostic::from_raw(ffi::RawSyntaxDiagnostic {
            code: code.code_raw(),
            subsystem: code.subsystem_raw(),
            severity: DiagnosticSeverity::Warning.as_raw(),
            message: String::new(),
            args: Vec::new(),
            name: "UnknownEscapeCode".to_owned(),
            option_name: "unknown-escape-code".to_owned(),
            primary_range_start: 0,
            primary_range_end: 0,
            has_primary_range: false,
            location: 0,
            has_location: false,
            buffer_id: 0,
            has_buffer_id: false,
            file_name: String::new(),
        });

        assert!(diagnostic.groups.contains(&"default".to_owned()));
    }

    #[test]
    fn syntax_tree_diagnostics_map_location_metadata() {
        let tree = SyntaxTree::from_text_with_options(
            r#"module demo; string s = "\q"; endmodule"#,
            "warning_demo",
            "warning_demo.sv",
            &Default::default(),
        );

        let diagnostics = tree.diagnostics(&[]);
        let diagnostic = diagnostics
            .iter()
            .find(|diag| {
                DiagCode::from_raw(diag.subsystem, diag.code) == DiagCode::UNKNOWN_ESCAPE_CODE
            })
            .expect("expected unknown escape code diagnostic");

        assert_eq!(diagnostic.file_name.as_deref(), Some("warning_demo.sv"));
        assert!(diagnostic.buffer_id.is_some(), "expected diagnostic buffer id");
        assert!(diagnostic.location.is_some(), "expected diagnostic location");
        assert!(!diagnostic.message.is_empty(), "expected formatted diagnostic message");
    }

    #[test]
    fn syntax_tree_diagnostics_apply_source_pragma_mappings() {
        let tree = SyntaxTree::from_text_with_options(
            "`pragma diagnostic ignore=\"unknown-escape-code\"\nmodule warning_demo; string s = \"\\q\"; endmodule",
            "pragma_demo",
            "pragma_demo.sv",
            &Default::default(),
        );

        let diagnostic = tree
            .diagnostics(&[])
            .into_iter()
            .find(|diag| {
                DiagCode::from_raw(diag.subsystem, diag.code) == DiagCode::UNKNOWN_ESCAPE_CODE
            })
            .expect("expected unknown escape code diagnostic");

        assert_eq!(diagnostic.severity, DiagnosticSeverity::Ignored);
    }
}
