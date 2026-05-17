use utils::line_index::TextSize;

use crate::{
    SyntaxNode, SyntaxToken,
    ast::{self, AstNode},
    has_text_range::HasTextRangeIn,
};

pub trait NamedConnectionDotZoneExt<'a> {
    fn dot_name_zone_contains(&self, offset: TextSize) -> bool;
}

fn dot_name_zone_contains_impl<'a>(
    offset: TextSize,
    context: SyntaxNode<'a>,
    dot: Option<SyntaxToken<'a>>,
    name: Option<SyntaxToken<'a>>,
    open_paren: Option<SyntaxToken<'a>>,
) -> bool {
    let text_range = |token: SyntaxToken<'a>| token.text_range_in(context);
    let Some(dot) = dot else {
        return false;
    };
    let Some(dot_range) = text_range(dot) else {
        return false;
    };

    let zone_end = open_paren
        .and_then(text_range)
        .map(|r| r.start())
        .or_else(|| name.and_then(text_range).map(|r| r.end()))
        .unwrap_or_else(|| dot_range.end());

    offset >= dot_range.end() && offset <= zone_end
}

impl<'a> NamedConnectionDotZoneExt<'a> for ast::NamedPortConnection<'a> {
    fn dot_name_zone_contains(&self, offset: TextSize) -> bool {
        dot_name_zone_contains_impl(
            offset,
            self.syntax(),
            self.dot(),
            self.name(),
            self.open_paren(),
        )
    }
}

impl<'a> NamedConnectionDotZoneExt<'a> for ast::NamedParamAssignment<'a> {
    fn dot_name_zone_contains(&self, offset: TextSize) -> bool {
        dot_name_zone_contains_impl(
            offset,
            self.syntax(),
            self.dot(),
            self.name(),
            self.open_paren(),
        )
    }
}
