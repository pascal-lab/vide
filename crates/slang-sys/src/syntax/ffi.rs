#![allow(non_snake_case)]
#![allow(clippy::module_inception)]
#![allow(clippy::too_many_arguments)]

#[allow(unused_imports)]
use cxx::SharedPtr;
pub(crate) use slang_ffi::*;

#[cxx::bridge(namespace = "slang_sys::syntax")]
mod slang_ffi {
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        type SyntaxTree;
        type SyntaxNode;
        type SyntaxToken;
        type SyntaxTrivia;
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
        ) -> SharedPtr<SyntaxTree>;
        fn syntax_tree_root(tree: &SyntaxTree) -> *const SyntaxNode;
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
        ) -> bool;
        unsafe fn syntax_node_range_with_context_start_buffer_id(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
        ) -> u32;
        unsafe fn syntax_node_range_with_context_start_offset(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
        ) -> usize;
        unsafe fn syntax_node_range_with_context_end_buffer_id(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
        ) -> u32;
        unsafe fn syntax_node_range_with_context_end_offset(
            node: *const SyntaxNode,
            context: *const SyntaxNode,
        ) -> usize;
        unsafe fn syntax_node_parent(node: *const SyntaxNode) -> *const SyntaxNode;
        unsafe fn syntax_node_child_count(node: *const SyntaxNode) -> usize;
        unsafe fn syntax_node_list_child_count(node: *const SyntaxNode) -> usize;
        unsafe fn syntax_node_list_child_size(node: *const SyntaxNode, index: usize) -> usize;
        unsafe fn syntax_node_child_node(
            node: *const SyntaxNode,
            index: usize,
        ) -> *const SyntaxNode;
        unsafe fn syntax_node_child_token(
            node: *const SyntaxNode,
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
        ) -> bool;
        unsafe fn syntax_token_range_with_context_start_buffer_id(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
        ) -> u32;
        unsafe fn syntax_token_range_with_context_start_offset(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
        ) -> usize;
        unsafe fn syntax_token_range_with_context_end_buffer_id(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
        ) -> u32;
        unsafe fn syntax_token_range_with_context_end_offset(
            token: *const SyntaxToken,
            context: *const SyntaxNode,
        ) -> usize;
        unsafe fn syntax_token_value_text(token: *const SyntaxToken) -> String;
        unsafe fn syntax_token_trivia_count(token: *const SyntaxToken) -> usize;
        unsafe fn syntax_token_trivia(
            token: *const SyntaxToken,
            index: usize,
        ) -> *const SyntaxTrivia;
    }

    #[namespace = "slang_sys::syntax::trivia"]
    unsafe extern "C++" {
        include!("syntax/wrapper.h");

        unsafe fn syntax_trivia_kind(trivia: *const SyntaxTrivia) -> u8;
        unsafe fn syntax_trivia_raw_text(trivia: *const SyntaxTrivia) -> String;
        unsafe fn syntax_trivia_syntax(trivia: *const SyntaxTrivia) -> *const SyntaxNode;
        unsafe fn syntax_trivia_explicit_location_valid(trivia: *const SyntaxTrivia) -> bool;
        unsafe fn syntax_trivia_explicit_location_buffer_id(trivia: *const SyntaxTrivia) -> u32;
        unsafe fn syntax_trivia_explicit_location_offset(trivia: *const SyntaxTrivia) -> usize;
    }
}
