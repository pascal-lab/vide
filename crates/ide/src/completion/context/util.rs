use syntax::{SyntaxNode, SyntaxToken, has_text_range::HasTextRangeIn};
use utils::line_index::TextSize;

pub(super) fn in_parens<'a>(
    offset: TextSize,
    open_paren: Option<SyntaxToken<'a>>,
    close_paren: Option<SyntaxToken<'a>>,
    owner: SyntaxNode<'a>,
) -> bool {
    let Some(open) = open_paren else {
        return false;
    };
    let Some(open_range) = open.text_range_in(owner) else {
        return false;
    };

    let close_start = close_paren
        .and_then(|t| t.text_range_in(owner))
        .map(|r| r.start())
        .unwrap_or_else(|| TextSize::from(u32::MAX));

    offset >= open_range.end() && offset <= close_start
}
