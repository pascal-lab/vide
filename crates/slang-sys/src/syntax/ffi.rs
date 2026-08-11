#![allow(non_snake_case)]
#![allow(clippy::module_inception)]
#![allow(clippy::too_many_arguments)]

#[allow(unused_imports)]
use cxx::SharedPtr;
pub(crate) use slang_ffi::*;

#[cxx::bridge(namespace = "slang_sys::syntax")]
mod slang_ffi {
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceSourceBuffer {
        path: String,
        text: String,
        buffer_id: u32,
        origin: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceSourceRange {
        buffer_id: u32,
        range_start: usize,
        range_end: usize,
        has_range: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceToken {
        raw_text: String,
        value_text: String,
        token_kind: u16,
        range: RawTraceSourceRange,
        has_token: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceMacroParam {
        name: RawTraceToken,
        default_tokens: Vec<RawTraceToken>,
        has_default: bool,
        range: RawTraceSourceRange,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceActualArgument {
        tokens: Vec<RawTraceToken>,
        range: RawTraceSourceRange,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceEvent {
        event_id: u32,
        kind: u16,
        range: RawTraceSourceRange,
        macro_origin: u8,
        macro_definition_id: u32,
        has_macro_definition_id: bool,
        macro_call_id: u32,
        has_macro_call_id: bool,
        macro_expansion_id: u32,
        has_macro_expansion_id: bool,
        parent_macro_expansion_id: u32,
        has_parent_macro_expansion_id: bool,
        directive: RawTraceToken,
        name: RawTraceToken,
        include_file_name: RawTraceToken,
        params: Vec<RawTraceMacroParam>,
        arguments: Vec<RawTraceActualArgument>,
        body_tokens: Vec<RawTraceToken>,
        expr_tokens: Vec<RawTraceToken>,
        disabled_ranges: Vec<RawTraceSourceRange>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceTokenOrigin {
        kind: u8,
        macro_name: String,
        macro_call_id: u32,
        has_macro_call_id: bool,
        macro_definition_id: u32,
        has_macro_definition_id: bool,
        macro_expansion_id: u32,
        has_macro_expansion_id: bool,
        parent_macro_expansion_id: u32,
        has_parent_macro_expansion_id: bool,
        body_token_index: u32,
        has_body_token_index: bool,
        argument_index: u32,
        has_argument_index: bool,
        argument_token_index: u32,
        has_argument_token_index: bool,
        token_range: RawTraceSourceRange,
        call_range: RawTraceSourceRange,
        body_token_range: RawTraceSourceRange,
        argument_token_range: RawTraceSourceRange,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceEmittedToken {
        emitted_token_index: u32,
        has_emitted_token_index: bool,
        raw_text: String,
        value_text: String,
        display_text: String,
        token_kind: u16,
        origin: RawTraceTokenOrigin,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTraceIncludeEdge {
        include_event_id: u32,
        included_buffer_id: u32,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawTrace {
        root_buffer_id: u32,
        source_buffers: Vec<RawTraceSourceBuffer>,
        events: Vec<RawTraceEvent>,
        include_edges: Vec<RawTraceIncludeEdge>,
        emitted_tokens: Vec<RawTraceEmittedToken>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RawSVInt {
        bit_width: u32,
        is_signed: bool,
        has_unknown: bool,
        single_word: u64,
        has_single_word: bool,
        binary: String,
        octal: String,
        decimal: String,
        hexadecimal: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RawOptionalU32 {
        value: u32,
        has_value: bool,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RawExpectedSyntax {
        code: u16,
        subsystem: u16,
        token_kind: u16,
        keyword_context: u8,
        has_keyword_context: bool,
        location: usize,
        end: usize,
        has_location: bool,
        has_end: bool,
        buffer_id: u32,
        has_buffer_id: bool,
    }

    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        type SyntaxTree;
        type SyntaxNode;
        type SyntaxToken;
    }

    #[namespace = "slang_sys::syntax::tree"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        fn parse_syntax_tree(
            text: &str,
            name: &str,
            path: &str,
            predefines: Vec<String>,
            include_paths: Vec<String>,
            include_buffer_paths: Vec<String>,
            include_buffer_texts: Vec<String>,
            expand_includes: bool,
            guess: bool,
            collect_expected_syntax: bool,
            expected_syntax_offset: usize,
            has_expected_syntax_offset: bool,
        ) -> SharedPtr<SyntaxTree>;
        fn syntax_tree_root(tree: &SyntaxTree) -> *const SyntaxNode;
        fn syntax_tree_root_buffer_id(tree: &SyntaxTree) -> u32;
        fn syntax_tree_expected_syntax_at(
            tree: &SyntaxTree,
            offset: usize,
        ) -> Vec<RawExpectedSyntax>;
        fn parse_library_map_syntax_tree(
            text: &str,
            name: &str,
            path: &str,
            collect_expected_syntax: bool,
            expected_syntax_offset: usize,
            has_expected_syntax_offset: bool,
        ) -> SharedPtr<SyntaxTree>;
        fn syntax_tree_preprocessor_trace(tree: &SyntaxTree) -> RawTrace;
    }

    #[namespace = "slang_sys::syntax::node"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        unsafe fn syntax_node_kind(node: *const SyntaxNode) -> u16;
        unsafe fn syntax_node_range_valid(node: *const SyntaxNode) -> bool;
        unsafe fn syntax_node_range_start_buffer_id(node: *const SyntaxNode) -> u32;
        unsafe fn syntax_node_range_start_offset(node: *const SyntaxNode) -> usize;
        unsafe fn syntax_node_range_end_buffer_id(node: *const SyntaxNode) -> u32;
        unsafe fn syntax_node_range_end_offset(node: *const SyntaxNode) -> usize;
        unsafe fn syntax_node_range_with_context_valid(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> bool;
        unsafe fn syntax_node_range_with_context_start_buffer_id(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> u32;
        unsafe fn syntax_node_range_with_context_start_offset(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> usize;
        unsafe fn syntax_node_range_with_context_end_buffer_id(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> u32;
        unsafe fn syntax_node_range_with_context_end_offset(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> usize;
        unsafe fn syntax_node_parent(node: *const SyntaxNode) -> *const SyntaxNode;
        unsafe fn syntax_node_child_count(node: *const SyntaxNode) -> usize;
        unsafe fn syntax_node_list_child_count(node: *mut SyntaxNode) -> usize;
        unsafe fn syntax_node_list_child_size(node: *mut SyntaxNode, index: usize) -> usize;
        unsafe fn syntax_node_child_node(
            node: *const SyntaxNode,
            index: usize,
        ) -> *const SyntaxNode;
        unsafe fn syntax_node_child_token(
            node: *mut SyntaxNode,
            index: usize,
        ) -> *const SyntaxToken;
    }

    #[namespace = "slang_sys::syntax::token"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        unsafe fn syntax_token_kind(token: *const SyntaxToken) -> u16;
        unsafe fn syntax_token_range_valid(token: *const SyntaxToken) -> bool;
        unsafe fn syntax_token_range_start_buffer_id(token: *const SyntaxToken) -> u32;
        unsafe fn syntax_token_range_start_offset(token: *const SyntaxToken) -> usize;
        unsafe fn syntax_token_range_end_buffer_id(token: *const SyntaxToken) -> u32;
        unsafe fn syntax_token_range_end_offset(token: *const SyntaxToken) -> usize;
        unsafe fn syntax_token_range_with_context_valid(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> bool;
        unsafe fn syntax_token_range_with_context_start_buffer_id(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> u32;
        unsafe fn syntax_token_range_with_context_start_offset(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> usize;
        unsafe fn syntax_token_range_with_context_end_buffer_id(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> u32;
        unsafe fn syntax_token_range_with_context_end_offset(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> usize;
        unsafe fn syntax_token_value_text(token: *const SyntaxToken) -> String;
        unsafe fn syntax_token_raw_text(token: *const SyntaxToken) -> String;
        unsafe fn syntax_token_int_value(token: *const SyntaxToken) -> RawSVInt;
        unsafe fn syntax_token_real_value(token: *const SyntaxToken) -> f64;
        unsafe fn syntax_token_bit_value(token: *const SyntaxToken) -> u8;
        unsafe fn syntax_token_literal_base(token: *const SyntaxToken) -> u8;
        unsafe fn syntax_token_time_unit(token: *const SyntaxToken) -> u8;
        unsafe fn syntax_token_preprocessor_trace_emitted_token_index(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> RawOptionalU32;
        unsafe fn syntax_token_preprocessor_trace_emitted_token(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
            owner: &SyntaxTree,
        ) -> RawTraceEmittedToken;
        unsafe fn syntax_token_trivia_count(token: *const SyntaxToken) -> usize;
    }

    #[namespace = "slang_sys::syntax::trivia"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        unsafe fn syntax_trivia_kind(token: *const SyntaxToken, index: usize) -> u8;
        unsafe fn syntax_trivia_raw_text(token: *const SyntaxToken, index: usize) -> String;
        unsafe fn syntax_trivia_syntax(
            token: *const SyntaxToken,
            index: usize,
        ) -> *const SyntaxNode;
        unsafe fn syntax_trivia_explicit_location_valid(
            token: *const SyntaxToken,
            index: usize,
        ) -> bool;
        unsafe fn syntax_trivia_explicit_location_buffer_id(
            token: *const SyntaxToken,
            index: usize,
        ) -> u32;
        unsafe fn syntax_trivia_explicit_location_offset(
            token: *const SyntaxToken,
            index: usize,
        ) -> usize;
    }

    #[namespace = "slang_sys::syntax::token"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        fn syntax_token_keyword_table_for_version(version: &str) -> Vec<String>;
        fn syntax_token_keyword_kind_for_version(version: &str, text: &str) -> u16;
        fn syntax_token_directive_kind(text: &str) -> u16;
        fn syntax_token_directive_text(kind: u16) -> String;
    }

    #[namespace = "slang_sys::syntax::facts"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        fn is_possible_statement(kind: u16) -> bool;
        fn is_possible_expression(kind: u16) -> bool;
        fn is_possible_data_type(kind: u16) -> bool;
        fn is_possible_argument(kind: u16) -> bool;
        fn is_possible_param_assignment(kind: u16) -> bool;
        fn is_possible_port_connection(kind: u16) -> bool;
        fn is_possible_ansi_port(kind: u16) -> bool;
        fn is_possible_non_ansi_port(kind: u16) -> bool;
        fn is_possible_function_port(kind: u16) -> bool;
        fn is_possible_parameter(kind: u16) -> bool;
        fn is_gate_type(kind: u16) -> bool;
        fn is_edge_kind(kind: u16) -> bool;
        fn is_port_direction(kind: u16) -> bool;
        fn is_net_type(kind: u16) -> bool;
        fn get_integer_type(kind: u16) -> u16;
        fn get_keyword_type(kind: u16) -> u16;
        fn get_procedural_block_kind(kind: u16) -> u16;
        fn get_module_declaration_kind(kind: u16) -> u16;
        fn is_possible_member_kind(token_kind: u16, member_kind: u16) -> bool;
        fn get_block_item_declaration_kind(kind: u16) -> u16;
        fn get_library_map_member_kind(kind: u16) -> u16;
        fn get_specify_item_kind(kind: u16) -> u16;
        fn get_config_header_item_kind(kind: u16) -> u16;
        fn get_config_rule_kind(kind: u16) -> u16;
        fn keyword_candidates_for_context(version: &str, context: u8) -> Vec<String>;
        fn is_allowed_in_compilation_unit(kind: u16) -> bool;
        fn is_allowed_in_generate(kind: u16) -> bool;
        fn is_allowed_in_module(kind: u16) -> bool;
        fn is_allowed_in_interface(kind: u16) -> bool;
        fn is_allowed_in_program(kind: u16) -> bool;
        fn is_allowed_in_package(kind: u16) -> bool;
    }
}
