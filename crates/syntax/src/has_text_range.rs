use slang::{
    SourceRange, SyntaxElement, SyntaxNode, SyntaxToken,
    SyntaxTokenWithParent as LocatedSyntaxToken,
};
use utils::line_index::TextRange;

pub(crate) trait SourceRangeExt {
    fn to_text_range(self) -> Option<TextRange>;
}

impl SourceRangeExt for SourceRange {
    #[inline]
    fn to_text_range(self) -> Option<TextRange> {
        if !self.is_single_buffer() {
            return None;
        }

        let start = u32::try_from(self.start()).ok()?;
        let end = u32::try_from(self.end()).ok()?;
        (start <= end).then(|| TextRange::new(start.into(), end.into()))
    }
}

pub trait HasTextRange {
    fn text_range(&self) -> Option<TextRange>;
}

/// Interpret a bare AST getter token in an explicit syntax context.
///
/// This is intended for the boundary where generated AST APIs still return a
/// raw [`SyntaxToken`]. Prefer [`HasTextRange`] on
/// [`slang::SyntaxTokenWithParent`] in IDE/HIR logic.
pub trait HasTextRangeIn<'a> {
    fn text_range_in(&self, context: SyntaxNode<'a>) -> Option<TextRange>;
}

impl HasTextRange for SyntaxNode<'_> {
    #[inline]
    fn text_range(&self) -> Option<TextRange> {
        self.range()?.to_text_range()
    }
}

impl<'a> HasTextRangeIn<'a> for SyntaxToken<'a> {
    #[inline]
    fn text_range_in(&self, context: SyntaxNode<'a>) -> Option<TextRange> {
        LocatedSyntaxToken { parent: context, tok: *self }.text_range()
    }
}

impl HasTextRange for LocatedSyntaxToken<'_> {
    #[inline]
    fn text_range(&self) -> Option<TextRange> {
        self.range()?.to_text_range()
    }
}

impl HasTextRange for SyntaxElement<'_> {
    #[inline]
    fn text_range(&self) -> Option<TextRange> {
        self.range()?.to_text_range()
    }
}
