//! This crate has re-exported the slang-sys crate APIs and provides some utils
//! based on slang-sys APIs.

// Utils
pub mod has_name;
pub mod has_text_range;
mod macros;
pub mod ptr;
pub mod slang_ext;
// Re-export slang-sys APIs
pub use slang_sys::{
    diagnostic::{
        DiagnosticSeverity, LexedTokenAtOffset, ParserExpectedSyntax, SyntaxDiagnostic,
        SyntaxKeywordContext,
    },
    source_buffer::{SourceLocation, SourceRange},
    syntax::{
        ChildrenIter, SyntaxAncestors, SyntaxChildren, SyntaxCursor, SyntaxElemPreorder,
        SyntaxElement, SyntaxElementKind, SyntaxIdxChildren, SyntaxKind, SyntaxNode,
        SyntaxNodePreorder, SyntaxToken, SyntaxTokenWithParent, SyntaxTree, SyntaxTreeBuffer,
        SyntaxTreeOptions, SyntaxTrivia, SyntaxTriviaLoc, WalkEvent, ast,
    },
    token::{TokenKind, TriviaKind},
};
