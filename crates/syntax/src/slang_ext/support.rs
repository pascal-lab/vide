use slang::{SyntaxNode, SyntaxToken, TokenKind, ast::AstNode};

#[inline]
pub fn child<'a, N: AstNode<'a>>(parent: SyntaxNode<'a>) -> Option<N> {
    parent.children().filter_map(|elem| elem.as_node()).find_map(N::cast)
}

#[inline]
pub fn child_token(parent: SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
    parent.children().filter_map(|elem| elem.as_token()).find(|tok| tok.kind() == kind)
}
