#![allow(non_snake_case)]
#![allow(clippy::module_inception)]

// These names are consumed by the cxx bridge DSL rather than Rust items.
#[allow(unused_imports)]
use std::pin::Pin;

// These names are consumed by the cxx bridge DSL rather than Rust items.
#[allow(unused_imports)]
use cxx::{SharedPtr, UniquePtr};
pub(crate) use slang_ffi::*;

#[cxx::bridge(namespace = "slang_sys::compilation")]
mod slang_ffi {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ParseSyntaxTreeOptions {
        predefines: Vec<String>,
        include_paths: Vec<String>,
        include_buffer_paths: Vec<String>,
        include_buffer_texts: Vec<String>,
        expand_includes: bool,
        collect_expected_syntax: bool,
        expected_syntax_offset: usize,
        has_expected_syntax_offset: bool,
    }

    #[namespace = "slang_sys::syntax"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        type SyntaxTree = crate::syntax::ffi::SyntaxTree;
    }

    #[namespace = "slang_sys::diagnostic"]
    unsafe extern "C++" {
        include!("slang-sys/src/diagnostic/ffi.rs.h");
        include!("diagnostic/wrapper.h");

        type RawSyntaxDiagnostic = crate::diagnostic::ffi::RawSyntaxDiagnostic;
    }

    unsafe extern "C++" {
        include!("compilation/wrapper.h");

        type Compilation;

        fn new_compilation(top_modules: Vec<String>) -> UniquePtr<Compilation>;
        fn parse_syntax_tree_from_text(
            compilation: Pin<&mut Compilation>,
            text: &str,
            name: &str,
            path: &str,
            options: ParseSyntaxTreeOptions,
        ) -> SharedPtr<SyntaxTree>;
        fn register_source_buffers(
            compilation: Pin<&mut Compilation>,
            paths: Vec<String>,
            texts: Vec<String>,
        );
        fn parse_syntax_tree_from_buffer(
            compilation: Pin<&mut Compilation>,
            name: &str,
            path: &str,
            options: ParseSyntaxTreeOptions,
        ) -> SharedPtr<SyntaxTree>;
        fn parse_library_map_syntax_tree_from_text(
            compilation: Pin<&mut Compilation>,
            text: &str,
            name: &str,
            path: &str,
        ) -> SharedPtr<SyntaxTree>;
        fn parse_library_map_syntax_tree_from_buffer(
            compilation: Pin<&mut Compilation>,
            name: &str,
            path: &str,
            collect_expected_syntax: bool,
            expected_syntax_offset: usize,
            has_expected_syntax_offset: bool,
        ) -> SharedPtr<SyntaxTree>;
        fn add_syntax_tree(compilation: Pin<&mut Compilation>, tree: SharedPtr<SyntaxTree>);
        fn parse_diagnostics(
            compilation: &Compilation,
            warning_options: Vec<String>,
        ) -> Vec<RawSyntaxDiagnostic>;
        fn semantic_diagnostics(
            compilation: &Compilation,
            warning_options: Vec<String>,
        ) -> Vec<RawSyntaxDiagnostic>;

    }
}
