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
    struct ClassMemberAnswer {
        found: bool,
        type_name: String,
        owner_class: String,
        inheritance: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct HierInstanceAnswer {
        path: String,
        file: String,
        offset: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SymbolAnswer {
        found: bool,
        name: String,
        type_name: String,
        kind: String,
        def_file: String,
        def_offset: usize,
        owner_class: String,
        inheritance: Vec<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MemberAnswer {
        name: String,
        type_name: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TypeAnswer {
        found: bool,
        type_name: String,
    }

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
        fn lookup_class_member(
            compilation: Pin<&mut Compilation>,
            path: &str,
            offset: usize,
        ) -> ClassMemberAnswer;
        fn lookup_symbol(
            compilation: Pin<&mut Compilation>,
            path: &str,
            offset: usize,
        ) -> SymbolAnswer;
        fn lookup_scoped(
            compilation: Pin<&mut Compilation>,
            left: &str,
            right: &str,
        ) -> SymbolAnswer;
        fn list_members(
            compilation: Pin<&mut Compilation>,
            path: &str,
            offset: usize,
        ) -> Vec<MemberAnswer>;
        fn list_scope_members(compilation: Pin<&mut Compilation>, name: &str) -> Vec<MemberAnswer>;
        fn lookup_type(
            compilation: Pin<&mut Compilation>,
            path: &str,
            start: usize,
            end: usize,
        ) -> TypeAnswer;
        fn list_instances(compilation: Pin<&mut Compilation>) -> Vec<HierInstanceAnswer>;

    }
}
