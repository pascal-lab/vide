#![allow(non_snake_case)]
#![allow(clippy::module_inception)]

pub(crate) use slang_ffi::*;

#[cxx::bridge(namespace = "slang_sys::diagnostic")]
mod slang_ffi {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawDiagnosticLocation {
        offset: usize,
        buffer_id: u32,
        has_location: bool,
        file_name: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawDiagnosticRange {
        start: usize,
        end: usize,
        start_buffer_id: u32,
        end_buffer_id: u32,
        has_range: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawDiagnosticExpansion {
        location: RawDiagnosticLocation,
        original_location: RawDiagnosticLocation,
        macro_name: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawSyntaxDiagnostic {
        code: u16,
        subsystem: u16,
        severity: u8,
        message: String,
        args: Vec<String>,
        name: String,
        option_name: String,
        primary_range_start: usize,
        primary_range_end: usize,
        has_primary_range: bool,
        location: usize,
        has_location: bool,
        buffer_id: u32,
        has_buffer_id: bool,
        file_name: String,
        ranges: Vec<RawDiagnosticRange>,
        expansion_locations: Vec<RawDiagnosticExpansion>,
        include_stack: Vec<RawDiagnosticLocation>,
        diagnostic_id: u32,
        parent_diagnostic_id: u32,
    }

    #[namespace = "slang_sys::syntax"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        type SyntaxTree = crate::syntax::ffi::SyntaxTree;
    }

    #[namespace = "slang_sys::diagnostic::tree"]
    unsafe extern "C++" {
        include!("diagnostic/wrapper.h");

        fn syntax_tree_diagnostics(
            tree: &SyntaxTree,
            warning_options: Vec<String>,
        ) -> Vec<RawSyntaxDiagnostic>;
    }
}
