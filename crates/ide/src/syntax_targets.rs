use syntax::{
    SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent, TokenKind, has_text_range::HasTextRange,
};
use utils::line_index::TextSize;

#[derive(Debug, Clone)]
pub(crate) struct SyntaxTarget<'tree> {
    tokens: Vec<SyntaxTokenWithParent<'tree>>,
}

impl<'tree> SyntaxTarget<'tree> {
    pub(crate) fn into_tokens(self) -> Vec<SyntaxTokenWithParent<'tree>> {
        self.tokens
    }
}

pub(crate) fn syntax_target_at_offset<'tree>(
    root: SyntaxNode<'tree>,
    offset: TextSize,
    precedence: impl Fn(TokenKind) -> usize,
) -> Option<SyntaxTarget<'tree>> {
    let token = root.token_at_offset(offset).pick_bext_token(precedence)?;
    token.text_range()?;
    Some(SyntaxTarget { tokens: vec![token] })
}
