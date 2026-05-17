use slang::{
    SourceRange, SyntaxElement, SyntaxNode, SyntaxToken,
    SyntaxTokenWithParent as LocatedSyntaxToken,
};
use utils::line_index::TextRange;

pub(crate) trait SourceRangeExt {
    fn to_text_range_in_root(self, root: SyntaxNode<'_>) -> Option<TextRange>;
}

impl SourceRangeExt for SourceRange {
    #[inline]
    fn to_text_range_in_root(self, root: SyntaxNode<'_>) -> Option<TextRange> {
        let root_range = root.range()?;
        if !root_range.is_single_buffer()
            || self.start_buffer_id() != root_range.start_buffer_id()
            || self.end_buffer_id() != root_range.start_buffer_id()
        {
            return None;
        }

        let start = u32::try_from(self.start()).ok()?;
        let end = u32::try_from(self.end()).ok()?;
        (start <= end).then(|| TextRange::new(start.into(), end.into()))
    }
}

fn root_node(mut node: SyntaxNode<'_>) -> SyntaxNode<'_> {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
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
        let root = root_node(*self);
        self.range_with_context(root)?.to_text_range_in_root(root)
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
        let root = root_node(self.parent);
        self.range()?.to_text_range_in_root(root)
    }
}

impl HasTextRange for SyntaxElement<'_> {
    #[inline]
    fn text_range(&self) -> Option<TextRange> {
        match self {
            SyntaxElement::Node(node) => node.text_range(),
            SyntaxElement::Token(token) => token.text_range(),
        }
    }
}
