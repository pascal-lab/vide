pub mod has_name;
pub mod has_text_range;
pub mod ptr;
pub mod slang_ext;

pub use slang::{
    ActualArgument, Bit, ChildrenIter, Compilation, CxxSV, DiagnosticSeverity, EmittedToken, Event,
    EventId, IncludeEdge, LexedTokenAtOffset, LiteralBase, MacroCallId, MacroDefinitionId,
    MacroExpansionId, MacroParam, ParserExpectedSyntax, SVInt, SVLogic, SemanticFacts,
    SourceBufferId, SourceBufferOrigin, SourceBufferRange, SourceLocation, SourceRange,
    SyntaxAncestors, SyntaxChildren, SyntaxCursor, SyntaxDiagnostic, SyntaxElemPreorder,
    SyntaxElement, SyntaxElementKind, SyntaxFacts, SyntaxIdxChildren, SyntaxKeywordContext,
    SyntaxKind, SyntaxNode, SyntaxNodePreorder, SyntaxToken, SyntaxTokenWithParent, SyntaxTree,
    SyntaxTreeBuffer, SyntaxTreeBufferIds, SyntaxTreeOptions, SyntaxTreeWithTrace, SyntaxTrivia,
    SyntaxTriviaIter, SyntaxTriviaLoc, TimeUnit, Token, TokenKind, TokenOrigin, Trace, Trivia,
    TriviaKind, WalkEvent, ast, preproc,
};
pub use slang_ext::*;

#[macro_export]
macro_rules! match_ast {
    ($node:expr , _ => $body:expr,) => { $body };

    ($node:expr , $path:ty[$it:pat] $(if $cond:expr)? => $body:expr, $($rest:tt)* ) => {{
        if let Some($it) = <$path as $crate::ast::AstNode>::cast($node)
        $( && ($cond) )? {
            $body
        } else {
            match_ast!($node , $($rest)*)
        }
    }};

    ($node:expr , $path:ty $(| $paths:ty)* => $body:expr, $($rest:tt)* ) => {{
        if <$path as $crate::ast::AstNode>::cast($node).is_some() $(|| <$paths as $crate::ast::AstNode>::cast($node).is_some())* {
            $body
        } else {
            match_ast!($node , $($rest)*)
        }
    }}
}

#[macro_export]
macro_rules! match_ast_kind {
    ($kind:expr , _ => $body:expr,) => { $body };

    ($kind:expr , $path:ty $(where $cond:expr)? => $body:expr, $($rest:tt)* ) => {{
        if <$path as $crate::ast::AstNode>::can_cast($kind)
        $( && ($cond) )? {
            $body
        } else {
            match_ast_kind!($kind , $($rest)*)
        }
    }};

    ($kind:expr , $path:ty $(| $paths:ty)* => $body:expr, $($rest:tt)* ) => {{
        if <$path as $crate::ast::AstNode>::can_cast($kind) $(|| <$paths as $crate::ast::AstNode>::can_cast($kind))* {
            $body
        } else {
            match_ast_kind!($kind , $($rest)*)
        }
    }}
}
