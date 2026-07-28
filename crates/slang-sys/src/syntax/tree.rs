use cxx::SharedPtr;

use super::{ffi, syntax_node::SyntaxNode};

/// An owned Slang syntax tree.
/// The tree owns the memory for nodes and tokens. Any `SyntaxNode` or
/// `SyntaxToken` borrowed from it must not outlive this value.
#[derive(Clone)]
pub struct SyntaxTree {
    pub(crate) raw: SharedPtr<ffi::SyntaxTree>,
}

/// Parser options for creating a syntax tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxTreeOptions {
    pub predefines: Vec<String>,
    pub include_paths: Vec<String>,
    pub include_buffers: Vec<SyntaxTreeBuffer>,
    pub expand_includes: bool,
}

/// In-memory source buffer that can be used for include resolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxTreeBuffer {
    pub path: String,
    pub text: String,
}

impl Default for SyntaxTreeOptions {
    fn default() -> Self {
        Self {
            predefines: Vec::new(),
            include_paths: Vec::new(),
            include_buffers: Vec::new(),
            expand_includes: true,
        }
    }
}

impl SyntaxTreeOptions {
    pub fn without_include_expansion() -> Self {
        Self { expand_includes: false, ..Self::default() }
    }
}

impl SyntaxTree {
    pub fn from_text(text: &str, name: &str, path: &str) -> Self {
        Self { raw: ffi::parse_syntax_tree(text, name, path) }
    }

    pub fn from_text_with_options(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> Self {
        Self {
            raw: ffi::parse_syntax_tree_with_options(
                text,
                name,
                path,
                options.predefines.clone(),
                options.include_paths.clone(),
                options.include_buffers.iter().map(|buffer| buffer.path.clone()).collect(),
                options.include_buffers.iter().map(|buffer| buffer.text.clone()).collect(),
                options.expand_includes,
            ),
        }
    }

    pub fn root(&self) -> Option<SyntaxNode<'_>> {
        let raw = ffi::syntax_tree_root(self.raw.as_ref()?);
        Some(SyntaxNode::from_nullable_raw(raw).expect("slang returned null syntax tree root"))
    }
}
