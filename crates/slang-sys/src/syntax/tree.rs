use std::fmt;

use cxx::SharedPtr;

use super::{ffi, syntax_node::SyntaxNode};
use crate::diagnostic::{
    LexedTokenAtOffset, ParserExpectedSyntax, SyntaxDiagnostic, ffi as diagnostic_ffi,
};

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
    pub fn from_text(text: &str, name: &str, path: &str, options: &SyntaxTreeOptions) -> Self {
        Self {
            raw: ffi::parse_syntax_tree(
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

    /// NOTE: This will only get diagnostic while parsing. For further
    /// diagnostics provided by slang, use Compilation structure.
    pub fn diagnostics(&self, warning_options: &[String]) -> Vec<SyntaxDiagnostic> {
        let Some(raw) = self.raw.as_ref() else {
            return Vec::new();
        };
        diagnostic_ffi::syntax_tree_diagnostics(raw, warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
    }

    pub fn expected_syntax_at_offset(
        _text: &str,
        _name: &str,
        _path: &str,
        _offset: usize,
    ) -> Vec<ParserExpectedSyntax> {
        unimplemented!("syntax tree completion queries are not ported yet")
    }

    pub fn expected_syntax_at_offset_with_options(
        _text: &str,
        _name: &str,
        _path: &str,
        _offset: usize,
        _options: &SyntaxTreeOptions,
    ) -> Vec<ParserExpectedSyntax> {
        unimplemented!("syntax tree completion queries are not ported yet")
    }

    pub fn library_map_expected_syntax_at_offset(
        _text: &str,
        _name: &str,
        _path: &str,
        _offset: usize,
    ) -> Vec<ParserExpectedSyntax> {
        unimplemented!("syntax tree completion queries are not ported yet")
    }

    pub fn directive_at_offset(
        _text: &str,
        _name: &str,
        _path: &str,
        _offset: usize,
    ) -> Option<LexedTokenAtOffset> {
        unimplemented!("directive completion queries are not ported yet")
    }

    pub fn token_word_at_offset(
        _text: &str,
        _name: &str,
        _path: &str,
        _offset: usize,
    ) -> Option<LexedTokenAtOffset> {
        unimplemented!("token word completion queries are not ported yet")
    }

    pub fn buffer_id(&self) -> u32 {
        self.root().and_then(|root| root.range()).map(|range| range.start_buffer_id()).unwrap_or(0)
    }
}

unsafe impl Send for SyntaxTree {}

unsafe impl Sync for SyntaxTree {}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxTree").finish()
    }
}

impl PartialEq for SyntaxTree {
    fn eq(&self, other: &Self) -> bool {
        self.root() == other.root()
    }
}

impl Eq for SyntaxTree {}
