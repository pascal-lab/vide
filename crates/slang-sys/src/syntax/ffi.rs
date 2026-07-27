#![allow(non_snake_case)]
#![allow(clippy::module_inception)]
#![allow(clippy::too_many_arguments)]

#[allow(unused_imports)]
use cxx::SharedPtr;
pub use slang_ffi::*;

#[cxx::bridge(namespace = "slang_sys::syntax")]
pub mod slang_ffi {
    unsafe extern "C++" {
        include!("wrapper.h");

        type SyntaxTree;
        type SyntaxNode;
        type SyntaxToken;

        fn parse_syntax_tree(text: &str, name: &str, path: &str) -> SharedPtr<SyntaxTree>;
        fn syntax_tree_root(tree: &SyntaxTree) -> *const SyntaxNode;

        unsafe fn syntax_node_kind(node: *const SyntaxNode) -> u16;
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

        unsafe fn syntax_token_kind(token: *const SyntaxToken) -> u16;
        unsafe fn syntax_token_value_text(token: *const SyntaxToken) -> String;
    }
}
