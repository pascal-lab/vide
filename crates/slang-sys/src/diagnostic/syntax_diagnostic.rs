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
    /// All ranges after Slang has mapped them into the reported context.
    ///
    /// `primary_range` is the single range currently consumed by the IDE
    /// model. These ranges retain cross-buffer information for consumers that
    /// need the full Slang report.
    pub ranges: Vec<SyntaxDiagnosticRange>,
    /// Macro expansion locations captured by `ReportedDiagnostic`.
    pub expansion_locations: Vec<SyntaxDiagnosticExpansion>,
    /// Include locations Slang marked for include-stack reporting.
    pub include_stack: Vec<SyntaxDiagnosticLocation>,
    /// Stable IDs assigned while a C++ diagnostic client collects reports.
    pub diagnostic_id: u32,
    pub parent_diagnostic_id: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnosticLocation {
    pub offset: usize,
    pub buffer_id: u32,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnosticRange {
    pub start: usize,
    pub end: usize,
    pub start_buffer_id: u32,
    pub end_buffer_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnosticExpansion {
    pub location: Option<SyntaxDiagnosticLocation>,
    pub original_location: Option<SyntaxDiagnosticLocation>,
    pub macro_name: String,
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

impl SyntaxKeywordContext {
    pub(crate) fn from_raw(raw: u8) -> Option<Self> {
        [
            Self::CompilationUnitMember,
            Self::LibraryMapMember,
            Self::ModuleHeaderItem,
            Self::ModuleMember,
            Self::GenerateMember,
            Self::SpecifyItem,
            Self::ConfigHeaderItem,
            Self::ConfigRule,
            Self::BlockItem,
            Self::Statement,
            Self::ParameterPortListItem,
            Self::AnsiPortItem,
            Self::FunctionPortItem,
            Self::GateType,
        ]
        .into_iter()
        .find(|context| *context as u8 == raw)
    }
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
            ranges: raw
                .ranges
                .into_iter()
                .filter(|range| range.has_range)
                .map(|range| SyntaxDiagnosticRange {
                    start: range.start,
                    end: range.end,
                    start_buffer_id: range.start_buffer_id,
                    end_buffer_id: range.end_buffer_id,
                })
                .collect(),
            expansion_locations: raw
                .expansion_locations
                .into_iter()
                .map(|expansion| SyntaxDiagnosticExpansion {
                    location: raw_location(expansion.location),
                    original_location: raw_location(expansion.original_location),
                    macro_name: expansion.macro_name,
                })
                .collect(),
            include_stack: raw
                .include_stack
                .into_iter()
                .filter_map(raw_location)
                .collect(),
            diagnostic_id: raw.diagnostic_id,
            parent_diagnostic_id: (raw.parent_diagnostic_id != 0)
                .then_some(raw.parent_diagnostic_id),
        }
    }
}

fn raw_location(raw: ffi::RawDiagnosticLocation) -> Option<SyntaxDiagnosticLocation> {
    raw.has_location.then(|| SyntaxDiagnosticLocation {
        offset: raw.offset,
        buffer_id: raw.buffer_id,
        file_name: (!raw.file_name.is_empty()).then_some(raw.file_name),
    })
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
            ranges: Vec::new(),
            expansion_locations: Vec::new(),
            include_stack: Vec::new(),
            diagnostic_id: 1,
            parent_diagnostic_id: 0,
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

        assert!(!tree.diagnostics(&[]).into_iter().any(|diag| {
            DiagCode::from_raw(diag.subsystem, diag.code) == DiagCode::UNKNOWN_ESCAPE_CODE
        }));
    }

    #[test]
    fn syntax_tree_diagnostics_report_unknown_warning_options() {
        let tree = SyntaxTree::from_text("module warning_demo; endmodule", "warning", "warning.sv");
        let diagnostics = tree.diagnostics(&["definitely-not-a-warning".to_owned()]);

        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.message.contains("warning")),
            "expected unknown warning option diagnostic, got {diagnostics:?}"
        );
    }

    #[test]
    fn syntax_tree_diagnostics_preserve_reported_macro_expansions() {
        let tree = SyntaxTree::from_text(
            "`define BAD (1 + )\nmodule demo; localparam int value = `BAD; endmodule",
            "macro_report",
            "macro_report.sv",
        );

        let diagnostic = tree
            .diagnostics(&[])
            .into_iter()
            .find(|diagnostic| !diagnostic.expansion_locations.is_empty())
            .expect("expected a diagnostic with a macro expansion chain");
        assert!(diagnostic.diagnostic_id != 0);
        assert!(diagnostic.expansion_locations.iter().any(|expansion| {
            expansion.macro_name == "BAD"
                && expansion.original_location.as_ref().is_some_and(|location| {
                    location.file_name.as_deref() == Some("macro_report.sv")
                })
        }));
    }

    #[test]
    fn syntax_tree_diagnostics_preserve_note_parent_relationships() {
        let tree = SyntaxTree::from_text(
            "module demo; always_comb begin case (1) default: ; default: ; endcase end endmodule",
            "note_report",
            "note_report.sv",
        );

        let diagnostics = tree.diagnostics(&[]);
        let note = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.parent_diagnostic_id.is_some())
            .expect("expected a diagnostic note");
        assert!(diagnostics.iter().any(|diagnostic| {
            Some(diagnostic.diagnostic_id) == note.parent_diagnostic_id
        }));
    }
}
