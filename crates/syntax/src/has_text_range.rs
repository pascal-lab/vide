use slang::{SourceRange, SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTokenWithParent};
use utils::line_index::TextRange;

pub trait SourceRangeExt {
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

impl HasTextRange for SyntaxNode<'_> {
    #[inline]
    fn text_range(&self) -> Option<TextRange> {
        self.range()?.to_text_range()
    }
}

impl HasTextRange for SyntaxToken<'_> {
    #[inline]
    fn text_range(&self) -> Option<TextRange> {
        self.range()?.to_text_range()
    }
}

impl HasTextRange for SyntaxTokenWithParent<'_> {
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
