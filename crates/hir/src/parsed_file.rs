//! High-level `ParsedFile` handle.
//!
//! Wraps `hir_semantics::ParsedFile` so IDE code parses a source file through
//! the facade and never names the `hir-semantics` type. The exposed surface
//! (`root`, `syntax_tree`, `compilation_unit`, `file`) is pure syntax plus the
//! facade `File`, so no implementation identifier leaks.

use syntax::{SyntaxNode, SyntaxTree, ast};

use crate::File;

/// A parsed source file: its syntax tree plus the [`File`] it came from.
pub struct ParsedFile {
    inner: hir_semantics::semantics::ParsedFile,
}

impl ParsedFile {
    pub(crate) fn new(inner: hir_semantics::semantics::ParsedFile) -> Self {
        Self { inner }
    }

    /// The file (real or macro expansion) this tree was parsed from.
    pub fn file(&self) -> File {
        self.inner.file_id().into()
    }

    /// The syntax tree. IDE code walks this with the `syntax` crate directly;
    /// `syntax` is a shared crate, not a HIR implementation layer.
    pub fn syntax_tree(&self) -> &SyntaxTree {
        self.inner.syntax_tree()
    }

    pub fn root(&self) -> Option<SyntaxNode<'_>> {
        self.inner.root()
    }

    pub fn compilation_unit(&self) -> Option<ast::CompilationUnit<'_>> {
        self.inner.compilation_unit()
    }
}