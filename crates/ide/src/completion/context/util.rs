use syntax::has_text_range::HasTextRange;
use utils::line_index::TextSize;

pub(super) fn in_parens(
    offset: TextSize,
    open_paren: Option<syntax::SyntaxToken<'_>>,
    close_paren: Option<syntax::SyntaxToken<'_>>,
) -> bool {
    let Some(open) = open_paren else {
        return false;
    };
    let Some(open_range) = open.text_range() else {
        return false;
    };

    let close_start = close_paren
        .and_then(|t| t.text_range())
        .map(|r| r.start())
        .unwrap_or_else(|| TextSize::from(u32::MAX));

    offset >= open_range.end() && offset <= close_start
}
