use syntax::{
    SyntaxNode, SyntaxNodeExt, TokenAtOffset, has_text_range::HasTextRange,
    token::SyntaxTokenWithParentExt,
};
use utils::line_index::{TextRange, TextSize};

pub(super) struct CaretSnapshot<'a> {
    pub(super) root: SyntaxNode<'a>,
    pub(super) offset: TextSize,
    token_at: TokenAtOffset<'a>,
    covering_node: Option<SyntaxNode<'a>>,
}

impl<'a> CaretSnapshot<'a> {
    pub(super) fn new(root: SyntaxNode<'a>, offset: TextSize) -> Self {
        let covering = root.covering_element(TextRange::empty(offset));
        let covering_node = covering.as_node().or_else(|| covering.parent());
        Self { root, offset, token_at: root.token_at_offset_including_eof(offset), covering_node }
    }

    pub(super) fn covering_node(&self) -> Option<SyntaxNode<'a>> {
        self.covering_node
    }

    pub(super) fn replacement_and_prefix(&self) -> (TextRange, String) {
        let tok_with_parent = match self.token_at.clone() {
            TokenAtOffset::Single(tok) => Some(tok),
            TokenAtOffset::Between(left, right) => {
                let left_range = left.text_range();
                if left_range.is_some_and(|r| r.end() == self.offset) {
                    Some(left)
                } else {
                    let right_range = right.text_range();
                    right_range.is_some_and(|r| r.start() == self.offset).then_some(right)
                }
            }
            TokenAtOffset::None => None,
        };

        let Some(tok_with_parent) = tok_with_parent else {
            return (TextRange::empty(self.offset), String::new());
        };

        if tok_with_parent.is_word_like() {
            let range =
                tok_with_parent.text_range().unwrap_or_else(|| TextRange::empty(self.offset));
            let prefix = if range.contains(self.offset) || range.end() == self.offset {
                let upto = usize::from(self.offset - range.start());
                let text = tok_with_parent.tok.raw_text().to_string();
                if upto <= text.len() && text.is_char_boundary(upto) {
                    text[..upto].to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            (range, prefix)
        } else {
            (TextRange::empty(self.offset), String::new())
        }
    }
}
