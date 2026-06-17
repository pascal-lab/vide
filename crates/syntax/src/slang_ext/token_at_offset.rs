use slang::{SyntaxTokenWithParent, TokenKind};

#[derive(Clone, Debug)]
pub enum TokenAtOffset<'a> {
    None,
    Single(SyntaxTokenWithParent<'a>),
    Between(SyntaxTokenWithParent<'a>, SyntaxTokenWithParent<'a>),
}

impl<'a> TokenAtOffset<'a> {
    #[inline]
    pub fn pick_bext_token(
        self,
        f: impl Fn(TokenKind) -> usize,
    ) -> Option<SyntaxTokenWithParent<'a>> {
        match self {
            TokenAtOffset::None => None,
            TokenAtOffset::Single(n) => Some(n),
            TokenAtOffset::Between(a, b) => {
                if f(a.kind()) > f(b.kind()) {
                    Some(a)
                } else {
                    Some(b)
                }
            }
        }
    }

    #[inline]
    pub fn left_biased(self) -> Option<SyntaxTokenWithParent<'a>> {
        match self {
            TokenAtOffset::None => None,
            TokenAtOffset::Single(node) => Some(node),
            TokenAtOffset::Between(left, _) => Some(left),
        }
    }
}

impl<'a> Iterator for TokenAtOffset<'a> {
    type Item = SyntaxTokenWithParent<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match std::mem::replace(self, TokenAtOffset::None) {
            TokenAtOffset::None => None,
            TokenAtOffset::Single(tok) => {
                *self = TokenAtOffset::None;
                Some(tok)
            }
            TokenAtOffset::Between(left, right) => {
                *self = TokenAtOffset::Single(right);
                Some(left)
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            TokenAtOffset::None => (0, Some(0)),
            TokenAtOffset::Single(_) => (1, Some(1)),
            TokenAtOffset::Between(_, _) => (2, Some(2)),
        }
    }
}
