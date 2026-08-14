//! This crate has re-exported the slang-sys crate APIs and provides some utils
//! based on slang-sys APIs.

// Utils
pub mod has_name;
pub mod has_text_range;
mod macros;
pub mod ptr;
mod slang_ext;

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
    AstNodeExt, SyntaxCursorExt, SyntaxNodeExt, SyntaxTokenExt, SyntaxTokenWithParentExt,
    TokenAtOffset, TokenKindExt, TriviaExt, TriviaKindExt, pair_token,
};
// Re-export slang-sys APIs
pub use slang_sys::{
    Bit, SVInt, SemanticFacts, SyntaxFacts, TimeUnit,
    compilation::Compilation,
    source_buffer::{SourceLocation, SourceRange},
    syntax::{
        ChildrenIter, SyntaxAncestors, SyntaxChildren, SyntaxCursor, SyntaxElemPreorder,
        SyntaxElement, SyntaxElementKind, SyntaxIdxChildren, SyntaxKind, SyntaxNode,
        SyntaxNodePreorder, SyntaxToken, SyntaxTokenWithParent, SyntaxTree, SyntaxTreeBuffer,
        SyntaxTreeOptions, SyntaxTrivia, SyntaxTriviaLoc, WalkEvent, ast,
    },
    token::{LiteralBase, TokenKind, TriviaKind},
};

pub mod diagnostics {
    pub use slang_sys::diagnostic::*;
}

pub mod preproc {
    pub use slang_sys::{
        preproc::*,
        source_buffer::{
            SourceBufferId, SourceBufferOrigin, SourceBufferRange, SyntaxTreeBufferIds,
        },
    };
}
