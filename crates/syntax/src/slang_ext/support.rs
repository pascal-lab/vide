use slang_sys::{
    syntax::{SyntaxNode, SyntaxToken},
    token::TokenKind,
};

#[inline]
pub fn child_token(parent: SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
    parent
        .children()
        .filter_map(|elem| elem.as_token())
        .find(|tok| tok.kind() == kind)
        .map(|tok| tok.tok)
}
