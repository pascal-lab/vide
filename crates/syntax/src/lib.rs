//! This crate has re-exported the slang-sys crate APIs and provides some utils
//! based on slang-sys APIs.

// Utils
pub mod has_name;
pub mod has_text_range;
mod macros;
pub mod ptr;
mod slang_ext;

pub mod token {
    pub use slang_sys::token::{TokenKind, TriviaKind};

    pub use crate::slang_ext::token::{
        SyntaxTokenExt, SyntaxTokenWithParentExt, TokenKindExt, pair_token,
    };
}

pub mod trivia {
    pub use slang_sys::token::TriviaKind;

    pub use crate::slang_ext::trivia::{TriviaExt, TriviaKindExt};
}

pub type Trivia<'a> = SyntaxTrivia<'a>;
pub use slang_ext::{
    AstNodeExt, SyntaxCursorExt, SyntaxNodeExt, SyntaxTokenExt, SyntaxTokenWithParentExt,
    TokenAtOffset, TokenKindExt, TriviaExt, TriviaKindExt, pair_token,
};
pub use slang_sys::{
    syntax::{
        ChildrenIter, SyntaxAncestors, SyntaxChildren, SyntaxCursor, SyntaxElemPreorder,
        SyntaxElement, SyntaxElementKind, SyntaxIdxChildren, SyntaxKind, SyntaxNode,
        SyntaxNodePreorder, SyntaxToken, SyntaxTokenWithParent, SyntaxTree, SyntaxTreeBuffer,
        SyntaxTreeOptions, SyntaxTrivia, SyntaxTriviaLoc, WalkEvent, ast,
    },
    token::{TokenKind, TriviaKind},
};

pub mod compilation {
    pub use slang_sys::compilation::Compilation;
}

pub mod facts {
    pub use slang_sys::{SemanticFacts, SyntaxFacts};
}

pub mod source {
    pub use slang_sys::source_buffer::{SourceLocation, SourceRange};
}

pub mod value {
    pub use slang_sys::{Bit, LiteralBase, SVInt, TimeUnit};
}

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
