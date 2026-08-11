//! This crate has re-exported the slang-sys crate APIs and provides some utils
//! based on slang-sys APIs.

// Utils
pub mod has_name;
pub mod has_text_range;
mod macros;
pub mod ptr;
pub mod slang_ext;

/// Compatibility namespace for extension traits and token metadata.
pub mod token {
    pub use slang_sys::token::*;

    pub use crate::slang_ext::token::*;
}

/// Compatibility namespace for trivia extension traits.
pub mod trivia {
    pub use slang_sys::token::TriviaKind;

    pub use crate::slang_ext::trivia::*;
}

pub type Trivia<'a> = SyntaxTrivia<'a>;
pub use slang_ext::{
    AstNodeExt, NamedConnectionDotZoneExt, SyntaxCursorExt, SyntaxNodeExt, SyntaxTokenExt,
    SyntaxTokenWithParentExt, TokenAtOffset, TokenKindExt, TriviaExt, TriviaKindExt, ast_ext,
    pair_token,
};
pub use slang_sys::{
    ActualArgument, Event, EventId, MacroCallId, MacroDefinitionId, MacroExpansionId, MacroParam,
    SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxTreeBufferIds, Token, TokenOrigin,
    Trace,
};
// Re-export slang-sys APIs
pub use slang_sys::{
    Bit, SVInt, SemanticFacts, SyntaxFacts, TimeUnit,
    compilation::Compilation,
    diagnostic::{
        DiagCode, DiagnosticSeverity, LexedTokenAtOffset, ParserExpectedSyntax, SyntaxDiagnostic,
        SyntaxDiagnosticExpansion, SyntaxDiagnosticLocation, SyntaxDiagnosticRange,
        SyntaxKeywordContext,
    },
    source_buffer::{SourceLocation, SourceRange},
    syntax::{
        ChildrenIter, SyntaxAncestors, SyntaxChildren, SyntaxCursor, SyntaxElemPreorder,
        SyntaxElement, SyntaxElementKind, SyntaxIdxChildren, SyntaxKind, SyntaxNode,
        SyntaxNodePreorder, SyntaxToken, SyntaxTokenWithParent, SyntaxTree, SyntaxTreeBuffer,
        SyntaxTreeOptions, SyntaxTrivia, SyntaxTriviaLoc, WalkEvent, ast,
    },
    token::{LiteralBase, TokenKind, TriviaKind},
};

pub mod preproc {
    pub use slang_sys::preproc::*;
}
