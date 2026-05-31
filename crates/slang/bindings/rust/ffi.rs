#![allow(non_snake_case)]
#![allow(clippy::module_inception)]
#![allow(clippy::too_many_arguments)]

mod cxx_sv;

pub use std::pin::Pin;

use cxx::{SharedPtr, UniquePtr};
pub use cxx_sv::CxxSV;
pub use slang_ffi::*;

#[cxx::bridge]
mod slang_ffi {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawSyntaxDiagnostic {
        code: u16,
        subsystem: u16,
        severity: u8,
        message: String,
        args: Vec<String>,
        name: String,
        option_name: String,
        groups: Vec<String>,
        primary_range_start: usize,
        primary_range_end: usize,
        has_primary_range: bool,
        location: usize,
        has_location: bool,
        buffer_id: u32,
        has_buffer_id: bool,
        file_name: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawSourceBuffer {
        path: String,
        text: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawSourceBufferId {
        path: String,
        buffer_id: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawSyntaxTreeBufferIds {
        root_buffer_id: u32,
        source_buffers: Vec<RawSourceBufferId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawExpectedSyntax {
        code: u16,
        subsystem: u16,
        name: String,
        token_kind: u16,
        keyword_context: u8,
        has_keyword_context: bool,
        location: usize,
        has_location: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawLexedTokenAtOffset {
        replacement_start: usize,
        replacement_end: usize,
        prefix: String,
        token_kind: u16,
        directive_kind: u16,
        has_directive_kind: bool,
        has_token: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawPreprocessorToken {
        raw_text: String,
        value_text: String,
        range_start: usize,
        range_end: usize,
        has_range: bool,
        has_token: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTextRange {
        range_start: usize,
        range_end: usize,
        has_range: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawPreprocessorMacroParam {
        name: RawPreprocessorToken,
        default_tokens: Vec<RawPreprocessorToken>,
        has_default: bool,
        range_start: usize,
        range_end: usize,
        has_range: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawPreprocessorDirective {
        kind: u16,
        range_start: usize,
        range_end: usize,
        has_range: bool,
        directive: RawPreprocessorToken,
        name: RawPreprocessorToken,
        include_file_name: RawPreprocessorToken,
        params: Vec<RawPreprocessorMacroParam>,
        body_tokens: Vec<RawPreprocessorToken>,
        expr_tokens: Vec<RawPreprocessorToken>,
        disabled_ranges: Vec<RawTextRange>,
    }

    #[namespace = "slang"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        #[namespace = "wrapper"]
        type SourceLocation;

        fn offset(self: &SourceLocation) -> usize;

        #[namespace = "wrapper"]
        fn source_location_buffer_id(location: &SourceLocation) -> u32;

        #[namespace = "wrapper"]
        type SourceRange;

        #[namespace = "wrapper"]
        fn source_range_start(range: &SourceRange) -> usize;

        #[namespace = "wrapper"]
        fn source_range_end(range: &SourceRange) -> usize;

        #[namespace = "wrapper"]
        fn source_range_start_buffer_id(range: &SourceRange) -> u32;

        #[namespace = "wrapper"]
        fn source_range_end_buffer_id(range: &SourceRange) -> u32;
    }

    impl UniquePtr<SourceLocation> {}

    impl UniquePtr<SourceRange> {}

    #[namespace = "wrapper"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        #[cxx_name = "logic_t"]
        type SVLogic;

        fn isUnknown(self: &SVLogic) -> bool;

        fn toChar(self: &SVLogic) -> c_char;

        #[namespace = "wrapper"]
        fn logic_t_value(logic: &SVLogic) -> u8;

        type SVInt;

        fn isSigned(self: &SVInt) -> bool;

        fn hasUnknown(self: &SVInt) -> bool;

        fn getBitWidth(self: &SVInt) -> u32;

        fn getRawPtr(self: &SVInt) -> *const u64;

        #[namespace = "wrapper"]
        fn SVInt_toString(svint: &SVInt, base: usize) -> String;

        #[namespace = "wrapper"]
        fn SVInt_clone(svint: &SVInt) -> UniquePtr<SVInt>;

        #[namespace = "wrapper"]
        fn SVInt_eq(lhs: &SVInt, rhs: &SVInt) -> UniquePtr<SVLogic>;
    }

    impl UniquePtr<SVLogic> {}

    impl UniquePtr<SVInt> {}

    #[namespace = "wrapper::parsing"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        #[cxx_name = "Trivia"]
        type SyntaxTrivia;

        fn getRawText(self: &SyntaxTrivia) -> CxxSV<'_>;

        #[namespace = "wrapper::parsing"]
        fn SyntaxTrivia_kind(trivia: &SyntaxTrivia) -> u8;

        #[namespace = "wrapper::parsing"]
        fn SyntaxTrivia_syntax(trivia: &SyntaxTrivia) -> *const SyntaxNode;

        #[namespace = "wrapper::parsing"]
        fn SyntaxTrivia_getExplicitLocation(trivia: &SyntaxTrivia) -> UniquePtr<SourceLocation>;

        #[cxx_name = "Token"]
        type SyntaxToken;

        fn isMissing(self: &SyntaxToken) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_range(tok: &SyntaxToken) -> UniquePtr<SourceRange>;

        fn valueText(self: &SyntaxToken) -> CxxSV<'_>; // excapedText

        fn rawText(self: &SyntaxToken) -> CxxSV<'_>; // rawText

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_kind(tok: &SyntaxToken) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_intValue(tok: &SyntaxToken) -> UniquePtr<SVInt>;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_bitValue(tok: &SyntaxToken) -> UniquePtr<SVLogic>;

        fn realValue(self: &SyntaxToken) -> f64;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_base(tok: &SyntaxToken) -> u8;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_unit(tok: &SyntaxToken) -> u8;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_trivia_count(tok: &SyntaxToken) -> usize;

        #[namespace = "wrapper::parsing"]
        fn SyntaxToken_trivia(tok: &SyntaxToken, idx: usize) -> *const SyntaxTrivia;
    }

    #[namespace = "wrapper::parsing"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        #[namespace = "wrapper::parsing"]
        fn LexerFacts_keyword_table_for_version(version: CxxSV) -> Vec<String>;

        #[namespace = "wrapper::parsing"]
        fn LexerFacts_keyword_kind_for_version(version: CxxSV, text: CxxSV) -> u16;

        #[namespace = "wrapper::parsing"]
        fn LexerFacts_verilog_2005_keywords() -> Vec<String>;

        #[namespace = "wrapper::parsing"]
        fn LexerFacts_directive_text(kind: u16) -> String;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_statement(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_expression(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_data_type(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_argument(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_param_assignment(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_port_connection(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_ansi_port(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_non_ansi_port(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_function_port(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_parameter(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_gate_type(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SemanticFacts_is_edge_kind(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_port_direction(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_net_type(kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_integer_type(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_keyword_type(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_procedural_block_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_module_declaration_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_is_possible_member_kind(token_kind: u16, member_kind: u16) -> bool;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_block_item_declaration_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_library_map_member_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_specify_item_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_config_header_item_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_get_config_rule_kind(kind: u16) -> u16;

        #[namespace = "wrapper::parsing"]
        fn SyntaxFacts_keyword_candidates_for_context(version: CxxSV, context: u8) -> Vec<String>;
    }

    impl UniquePtr<SyntaxTrivia> {}

    impl UniquePtr<SyntaxToken> {}

    #[namespace = "wrapper::syntax"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        type SyntaxNode;

        #[namespace = "wrapper::syntax"]
        fn SyntaxNode_range(node: &SyntaxNode) -> UniquePtr<SourceRange>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxNode_rangeWithContext(
            node: &SyntaxNode,
            context: &SyntaxNode,
        ) -> UniquePtr<SourceRange>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxToken_rangeWithContext(
            token: &SyntaxToken,
            context: &SyntaxNode,
        ) -> UniquePtr<SourceRange>;

        fn childNode(self: &SyntaxNode, idx: usize) -> *const SyntaxNode;

        #[namespace = "wrapper::syntax"]
        fn SyntaxNode_childToken(node: &SyntaxNode, idx: usize) -> *const SyntaxToken;

        #[namespace = "wrapper::syntax"]
        fn SyntaxNode_parent(node: &SyntaxNode) -> *const SyntaxNode;

        fn getChildCount(self: &SyntaxNode) -> usize;

        #[namespace = "wrapper::syntax"]
        fn SyntaxNode_kind(node: &SyntaxNode) -> u16;

        #[namespace = "wrapper::syntax"]
        fn SyntaxFacts_is_allowed_in_compilation_unit(kind: u16) -> bool;

        #[namespace = "wrapper::syntax"]
        fn SyntaxFacts_is_allowed_in_generate(kind: u16) -> bool;

        #[namespace = "wrapper::syntax"]
        fn SyntaxFacts_is_allowed_in_module(kind: u16) -> bool;

        #[namespace = "wrapper::syntax"]
        fn SyntaxFacts_is_allowed_in_interface(kind: u16) -> bool;

        #[namespace = "wrapper::syntax"]
        fn SyntaxFacts_is_allowed_in_program(kind: u16) -> bool;

        #[namespace = "wrapper::syntax"]
        fn SyntaxFacts_is_allowed_in_package(kind: u16) -> bool;
    }

    #[namespace = "wrapper::syntax"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        type SyntaxTree;
        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_fromText(text: CxxSV, name: CxxSV, path: CxxSV) -> SharedPtr<SyntaxTree>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_fromTextWithOptions(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            predefines: Vec<String>,
            include_paths: Vec<String>,
            include_buffers: Vec<RawSourceBuffer>,
            expand_includes: bool,
        ) -> SharedPtr<SyntaxTree>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_fromLibraryMapText(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
        ) -> SharedPtr<SyntaxTree>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_root(tree: &SyntaxTree) -> *const SyntaxNode;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_diagnostics(tree: &SyntaxTree) -> Vec<RawSyntaxDiagnostic>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_diagnostics_with_options(
            tree: &SyntaxTree,
            warning_options: Vec<String>,
        ) -> Vec<RawSyntaxDiagnostic>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_expectedSyntaxAtOffset(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            offset: usize,
            predefines: Vec<String>,
            include_paths: Vec<String>,
            include_buffers: Vec<RawSourceBuffer>,
            expand_includes: bool,
        ) -> Vec<RawExpectedSyntax>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_libraryMapExpectedSyntaxAtOffset(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            offset: usize,
        ) -> Vec<RawExpectedSyntax>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_directiveAtOffset(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            offset: usize,
        ) -> RawLexedTokenAtOffset;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_tokenWordAtOffset(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            offset: usize,
        ) -> RawLexedTokenAtOffset;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_preprocessorDirectives(
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            predefines: Vec<String>,
        ) -> Vec<RawPreprocessorDirective>;

        #[namespace = "wrapper::syntax"]
        fn SyntaxTree_buffer_id(tree: &SyntaxTree) -> u32;
    }

    impl SharedPtr<SyntaxTree> {}

    #[namespace = "wrapper::ast"]
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/wrapper.h");

        type Compilation;

        #[namespace = "wrapper::ast"]
        fn Compilation_new() -> UniquePtr<Compilation>;

        #[namespace = "wrapper::ast"]
        fn Compilation_new_with_top_modules(top_modules: Vec<String>) -> UniquePtr<Compilation>;

        #[namespace = "wrapper::ast"]
        fn Compilation_system_function_names() -> Vec<String>;

        #[namespace = "wrapper::ast"]
        fn Compilation_system_task_names() -> Vec<String>;

        #[namespace = "wrapper::ast"]
        fn Compilation_add_syntax_tree(
            compilation: Pin<&mut Compilation>,
            tree: SharedPtr<SyntaxTree>,
        );

        #[namespace = "wrapper::ast"]
        fn Compilation_add_syntax_tree_from_text(
            compilation: Pin<&mut Compilation>,
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
            predefines: Vec<String>,
            include_paths: Vec<String>,
            include_buffers: Vec<RawSourceBuffer>,
            expand_includes: bool,
        ) -> RawSyntaxTreeBufferIds;

        #[namespace = "wrapper::ast"]
        fn Compilation_add_library_map_syntax_tree_from_text(
            compilation: Pin<&mut Compilation>,
            text: CxxSV,
            name: CxxSV,
            path: CxxSV,
        ) -> RawSyntaxTreeBufferIds;

        #[namespace = "wrapper::ast"]
        fn Compilation_semantic_diagnostics(compilation: &Compilation) -> Vec<RawSyntaxDiagnostic>;

        #[namespace = "wrapper::ast"]
        fn Compilation_parse_diagnostics_with_options(
            compilation: &Compilation,
            warning_options: Vec<String>,
        ) -> Vec<RawSyntaxDiagnostic>;

        #[namespace = "wrapper::ast"]
        fn Compilation_semantic_diagnostics_with_options(
            compilation: &Compilation,
            warning_options: Vec<String>,
        ) -> Vec<RawSyntaxDiagnostic>;
    }

    impl UniquePtr<Compilation> {}

    // StringView
    unsafe extern "C++" {
        include!("slang/bindings/rust/ffi/string_view.h");

        #[namespace = "std"]
        #[cxx_name = "string_view"]
        type CxxSV<'a> = crate::CxxSV<'a>;
    }
}

macro_rules! forward_functions {
    (fn $name:ident(&self $(, $arg:ident: $arg_ty:ty)*) -> $ret:ty |> $ffi_fn:ident; $($tt:tt)*) => {
        #[inline]
        pub fn $name(&self $(, $arg: $arg_ty)*) -> $ret {
            slang_ffi::$ffi_fn(self $(, $arg)*)
        }
        forward_functions!($($tt)*);
    };
    (fn $name:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty |> $ffi_fn:ident; $($tt:tt)*) => {
        #[inline]
        pub fn $name($($arg: $arg_ty),*) -> $ret {
            slang_ffi::$ffi_fn($($arg),*)
        }
        forward_functions!($($tt)*);
    };
    () => {};
}

macro_rules! impl_functions {
    (impl $type:ty { $($tt:tt)* }) => {
        impl $type {forward_functions!($($tt)*); }
    };
}

impl_functions! {
    impl SourceLocation {
        fn buffer_id(&self) -> u32 |> source_location_buffer_id;
    }
}

impl_functions! {
    impl SourceRange {
        fn start(&self) -> usize |> source_range_start;
        fn end(&self) -> usize |> source_range_end;
        fn start_buffer_id(&self) -> u32 |> source_range_start_buffer_id;
        fn end_buffer_id(&self) -> u32 |> source_range_end_buffer_id;
    }
}

impl_functions! {
    impl SyntaxTree {
        fn fromText(text: CxxSV, name: CxxSV, path: CxxSV) -> SharedPtr<SyntaxTree> |> SyntaxTree_fromText;
        fn fromTextWithOptions(text: CxxSV, name: CxxSV, path: CxxSV, predefines: Vec<String>, include_paths: Vec<String>, include_buffers: Vec<RawSourceBuffer>, expand_includes: bool) -> SharedPtr<SyntaxTree> |> SyntaxTree_fromTextWithOptions;
        fn fromLibraryMapText(text: CxxSV, name: CxxSV, path: CxxSV) -> SharedPtr<SyntaxTree> |> SyntaxTree_fromLibraryMapText;
        fn root(&self) -> *const SyntaxNode |> SyntaxTree_root;
        fn diagnostics(&self) -> Vec<RawSyntaxDiagnostic> |> SyntaxTree_diagnostics;
        fn diagnostics_with_options(&self, warning_options: Vec<String>) -> Vec<RawSyntaxDiagnostic> |> SyntaxTree_diagnostics_with_options;
        fn expectedSyntaxAtOffset(text: CxxSV, name: CxxSV, path: CxxSV, offset: usize, predefines: Vec<String>, include_paths: Vec<String>, include_buffers: Vec<RawSourceBuffer>, expand_includes: bool) -> Vec<RawExpectedSyntax> |> SyntaxTree_expectedSyntaxAtOffset;
        fn libraryMapExpectedSyntaxAtOffset(text: CxxSV, name: CxxSV, path: CxxSV, offset: usize) -> Vec<RawExpectedSyntax> |> SyntaxTree_libraryMapExpectedSyntaxAtOffset;
        fn directiveAtOffset(text: CxxSV, name: CxxSV, path: CxxSV, offset: usize) -> RawLexedTokenAtOffset |> SyntaxTree_directiveAtOffset;
        fn tokenWordAtOffset(text: CxxSV, name: CxxSV, path: CxxSV, offset: usize) -> RawLexedTokenAtOffset |> SyntaxTree_tokenWordAtOffset;
        fn preprocessorDirectives(text: CxxSV, name: CxxSV, path: CxxSV, predefines: Vec<String>) -> Vec<RawPreprocessorDirective> |> SyntaxTree_preprocessorDirectives;
        fn buffer_id(&self) -> u32 |> SyntaxTree_buffer_id;
    }
}

impl_functions! {
    impl SyntaxNode {
        fn range(&self) -> UniquePtr<SourceRange> |> SyntaxNode_range;
        fn rangeWithContext(node: &SyntaxNode, context: &SyntaxNode) -> UniquePtr<SourceRange> |> SyntaxNode_rangeWithContext;
        fn kind(&self) -> u16 |> SyntaxNode_kind;
        fn childToken(&self, idx: usize) -> *const SyntaxToken |> SyntaxNode_childToken;
        fn parent(&self) -> *const SyntaxNode |> SyntaxNode_parent;
    }
}

impl_functions! {
    impl SyntaxToken {
        fn range(&self) -> UniquePtr<SourceRange> |> SyntaxToken_range;
        fn rangeWithContext(token: &SyntaxToken, context: &SyntaxNode) -> UniquePtr<SourceRange> |> SyntaxToken_rangeWithContext;
    }
}

impl_functions! {
    impl SyntaxTrivia {
        fn kind(&self) -> u8 |> SyntaxTrivia_kind;
        fn syntax(&self) -> *const SyntaxNode |> SyntaxTrivia_syntax;
        fn getExplicitLocation(&self) -> UniquePtr<SourceLocation> |> SyntaxTrivia_getExplicitLocation;
    }
}

impl_functions! {
    impl SyntaxToken {
        fn kind(&self) -> u16 |> SyntaxToken_kind;
        fn intValue(&self) -> UniquePtr<SVInt> |> SyntaxToken_intValue;
        fn bitValue(&self) -> UniquePtr<SVLogic> |> SyntaxToken_bitValue;
        fn base(&self) -> u8 |> SyntaxToken_base;
        fn unit(&self) -> u8 |> SyntaxToken_unit;
        fn trivia_count(&self) -> usize |> SyntaxToken_trivia_count;
        fn trivia(&self, idx: usize) -> *const SyntaxTrivia |> SyntaxToken_trivia;
    }
}

impl_functions! {
    impl SyntaxToken {
        fn keyword_table_for_version(version: CxxSV) -> Vec<String> |> LexerFacts_keyword_table_for_version;
        fn keyword_kind_for_version(version: CxxSV, text: CxxSV) -> u16 |> LexerFacts_keyword_kind_for_version;
        fn verilog_2005_keywords() -> Vec<String> |> LexerFacts_verilog_2005_keywords;
        fn directive_text(kind: u16) -> String |> LexerFacts_directive_text;
        fn is_possible_statement(kind: u16) -> bool |> SyntaxFacts_is_possible_statement;
        fn is_possible_expression(kind: u16) -> bool |> SyntaxFacts_is_possible_expression;
        fn is_possible_data_type(kind: u16) -> bool |> SyntaxFacts_is_possible_data_type;
        fn is_possible_argument(kind: u16) -> bool |> SyntaxFacts_is_possible_argument;
        fn is_possible_param_assignment(kind: u16) -> bool |> SyntaxFacts_is_possible_param_assignment;
        fn is_possible_port_connection(kind: u16) -> bool |> SyntaxFacts_is_possible_port_connection;
        fn is_possible_ansi_port(kind: u16) -> bool |> SyntaxFacts_is_possible_ansi_port;
        fn is_possible_non_ansi_port(kind: u16) -> bool |> SyntaxFacts_is_possible_non_ansi_port;
        fn is_possible_function_port(kind: u16) -> bool |> SyntaxFacts_is_possible_function_port;
        fn is_possible_parameter(kind: u16) -> bool |> SyntaxFacts_is_possible_parameter;
        fn is_gate_type(kind: u16) -> bool |> SyntaxFacts_is_gate_type;
        fn is_edge_kind(kind: u16) -> bool |> SemanticFacts_is_edge_kind;
        fn is_port_direction(kind: u16) -> bool |> SyntaxFacts_is_port_direction;
        fn is_net_type(kind: u16) -> bool |> SyntaxFacts_is_net_type;
        fn get_integer_type(kind: u16) -> u16 |> SyntaxFacts_get_integer_type;
        fn get_keyword_type(kind: u16) -> u16 |> SyntaxFacts_get_keyword_type;
        fn get_procedural_block_kind(kind: u16) -> u16 |> SyntaxFacts_get_procedural_block_kind;
        fn get_module_declaration_kind(kind: u16) -> u16 |> SyntaxFacts_get_module_declaration_kind;
        fn is_possible_member_kind(token_kind: u16, member_kind: u16) -> bool |> SyntaxFacts_is_possible_member_kind;
        fn get_block_item_declaration_kind(kind: u16) -> u16 |> SyntaxFacts_get_block_item_declaration_kind;
        fn get_library_map_member_kind(kind: u16) -> u16 |> SyntaxFacts_get_library_map_member_kind;
        fn get_specify_item_kind(kind: u16) -> u16 |> SyntaxFacts_get_specify_item_kind;
        fn get_config_header_item_kind(kind: u16) -> u16 |> SyntaxFacts_get_config_header_item_kind;
        fn get_config_rule_kind(kind: u16) -> u16 |> SyntaxFacts_get_config_rule_kind;
        fn keyword_candidates_for_context(version: CxxSV, context: u8) -> Vec<String> |> SyntaxFacts_keyword_candidates_for_context;
    }
}

impl_functions! {
    impl SyntaxNode {
        fn is_allowed_in_compilation_unit(kind: u16) -> bool |> SyntaxFacts_is_allowed_in_compilation_unit;
        fn is_allowed_in_generate(kind: u16) -> bool |> SyntaxFacts_is_allowed_in_generate;
        fn is_allowed_in_module(kind: u16) -> bool |> SyntaxFacts_is_allowed_in_module;
        fn is_allowed_in_interface(kind: u16) -> bool |> SyntaxFacts_is_allowed_in_interface;
        fn is_allowed_in_program(kind: u16) -> bool |> SyntaxFacts_is_allowed_in_program;
        fn is_allowed_in_package(kind: u16) -> bool |> SyntaxFacts_is_allowed_in_package;
    }
}

impl_functions! {
    impl SVLogic {
        fn value(&self) -> u8 |> logic_t_value;
    }
}

impl_functions! {
    impl SVInt {
        fn clone(&self) -> UniquePtr<SVInt> |> SVInt_clone;
        fn toString(&self, base: usize) -> String |> SVInt_toString;
        fn eq(&self, rhs: &SVInt) -> UniquePtr<SVLogic> |> SVInt_eq;
    }
}

impl_functions! {
    impl Compilation {
        fn new() -> UniquePtr<Compilation> |> Compilation_new;
        fn new_with_top_modules(top_modules: Vec<String>) -> UniquePtr<Compilation> |> Compilation_new_with_top_modules;
        fn system_function_names() -> Vec<String> |> Compilation_system_function_names;
        fn system_task_names() -> Vec<String> |> Compilation_system_task_names;
        fn add_syntax_tree(self_: Pin<&mut Compilation>, tree: SharedPtr<SyntaxTree>) -> () |> Compilation_add_syntax_tree;
        fn add_syntax_tree_from_text(self_: Pin<&mut Compilation>, text: CxxSV, name: CxxSV, path: CxxSV, predefines: Vec<String>, include_paths: Vec<String>, include_buffers: Vec<RawSourceBuffer>, expand_includes: bool) -> RawSyntaxTreeBufferIds |> Compilation_add_syntax_tree_from_text;
        fn add_library_map_syntax_tree_from_text(self_: Pin<&mut Compilation>, text: CxxSV, name: CxxSV, path: CxxSV) -> RawSyntaxTreeBufferIds |> Compilation_add_library_map_syntax_tree_from_text;
        fn semantic_diagnostics(&self) -> Vec<RawSyntaxDiagnostic> |> Compilation_semantic_diagnostics;
        fn parse_diagnostics_with_options(&self, warning_options: Vec<String>) -> Vec<RawSyntaxDiagnostic> |> Compilation_parse_diagnostics_with_options;
        fn semantic_diagnostics_with_options(&self, warning_options: Vec<String>) -> Vec<RawSyntaxDiagnostic> |> Compilation_semantic_diagnostics_with_options;
    }
}
