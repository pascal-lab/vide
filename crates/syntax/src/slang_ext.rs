use std::iter;

use either::Either;
use slang::{
    ChildrenIter, SyntaxAncestors, SyntaxCursor, SyntaxElement, SyntaxNode, SyntaxTokenWithParent,
    SyntaxTrivia, TokenKind, ast::AstNode,
};
use token::SyntaxTokenExt;
use utils::line_index::{TextRange, TextSize};

use crate::{has_text_range::HasTextRange, ptr::SyntaxNodePtr};

pub mod token;
pub mod trivia;

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

pub trait SyntaxNodeExt<'a> {
    fn elem_at_exact_range(&self, range: TextRange) -> Option<SyntaxElement<'a>>;
    fn covering_element(&self, range: TextRange) -> SyntaxElement<'a>;
    fn token_at_offset(&self, offset: TextSize) -> TokenAtOffset<'a>;
    fn token_or_node_at_offset(
        &self,
        offset: TextSize,
    ) -> Either<TokenAtOffset<'a>, SyntaxNode<'a>>;
    fn find_root(&self) -> SyntaxNode<'a>;
    fn to_ptr(&self) -> SyntaxNodePtr;
    fn trivias(&self) -> impl ChildrenIter<SyntaxTrivia<'a>> + use<'a, Self>;
    fn trivias_with_range(
        &self,
    ) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a, Self>;
}

impl<'a> SyntaxNodeExt<'a> for SyntaxNode<'a> {
    fn elem_at_exact_range(&self, range: TextRange) -> Option<SyntaxElement<'a>> {
        let start = range.start();
        let mut cursor = self.walk();
        loop {
            let elem = cursor.to_elem();
            let elem_range = elem.text_range()?;

            if !elem_range.contains_range(range) {
                return None;
            }

            if elem_range == range {
                return Some(elem);
            }

            cursor.goto_first_child_after_pos(start.into());
        }
    }

    fn covering_element(&self, range: TextRange) -> SyntaxElement<'a> {
        let start = range.start();

        let mut cursor = self.walk();
        loop {
            let elem = cursor.to_elem();

            if elem.text_range().is_none_or(|elem_range| !elem_range.contains_range(range)) {
                cursor.goto_parent();
                break cursor.to_elem();
            }

            match elem {
                SyntaxElement::Token(_) => break elem,
                SyntaxElement::Node(_) => {
                    if !cursor.goto_last_child_before_pos(start.into()) {
                        break elem;
                    }
                }
            }
        }
    }

    fn token_at_offset(&self, offset: TextSize) -> TokenAtOffset<'a> {
        let range = self.text_range().unwrap();
        if range.is_empty() || !(range.contains(offset)) {
            return TokenAtOffset::None;
        }

        let mut cursor = self.walk();
        cursor.goto_last_tok_before(offset);
        let left = cursor.to_tok_with_parent();
        let left_range = left.and_then(|left| left.text_range());
        if left_range.is_some_and(|range| range.contains(offset)) {
            return TokenAtOffset::Single(left.unwrap());
        }
        let left_ok = left_range.map(|range| range.end() == offset).unwrap_or(false);

        cursor.reset_to_root();
        cursor.goto_first_tok_after(offset);
        let right = cursor.to_tok_with_parent();
        let right_range = right.and_then(|right| right.text_range());
        let right_ok = right_range.map(|range| range.contains(offset)).unwrap_or(false);

        match (left_ok, right_ok) {
            (true, true) => TokenAtOffset::Between(left.unwrap(), right.unwrap()),
            (true, false) => TokenAtOffset::Single(left.unwrap()),
            (false, true) => TokenAtOffset::Single(right.unwrap()),
            (false, false) => TokenAtOffset::None,
        }
    }

    fn token_or_node_at_offset(
        &self,
        offset: TextSize,
    ) -> Either<TokenAtOffset<'a>, SyntaxNode<'a>> {
        let range = self.text_range().unwrap();
        if range.is_empty() || !(range.contains(offset)) {
            return Either::Left(TokenAtOffset::None);
        }

        let mut cursor = self.walk();
        cursor.goto_last_tok_before(offset);
        let left = cursor.to_tok_with_parent();
        let left_range = left.and_then(|left| left.text_range());
        if left_range.is_some_and(|range| range.contains(offset)) {
            return Either::Left(TokenAtOffset::Single(left.unwrap()));
        }
        let left_ok = left_range.map(|range| range.end() == offset).unwrap_or(false);

        cursor.reset_to_root();
        cursor.goto_first_tok_after(offset);
        let right = cursor.to_tok_with_parent();
        let right_range = right.and_then(|right| right.text_range());
        let right_ok = right_range.map(|range| range.contains(offset)).unwrap_or(false);

        match (left_ok, right_ok) {
            (true, true) => Either::Left(TokenAtOffset::Between(left.unwrap(), right.unwrap())),
            (true, false) => Either::Left(TokenAtOffset::Single(left.unwrap())),
            (false, true) => Either::Left(TokenAtOffset::Single(right.unwrap())),
            (false, false) => {
                while !cursor.to_elem().text_range().is_some_and(|range| range.contains(offset)) {
                    cursor.goto_parent();
                }
                Either::Right(cursor.to_node().unwrap())
            }
        }
    }

    #[inline]
    fn find_root(&self) -> SyntaxNode<'a> {
        SyntaxAncestors::start_from(*self).last().unwrap()
    }

    #[inline]
    fn to_ptr(&self) -> SyntaxNodePtr {
        SyntaxNodePtr::from_node(*self)
    }

    #[inline]
    fn trivias(&self) -> impl ChildrenIter<SyntaxTrivia<'a>> + use<'a> {
        if let Some(tok) = self.first_token() {
            Either::Right(tok.trivias())
        } else {
            Either::Left(iter::empty())
        }
    }

    #[inline]
    fn trivias_with_range(&self) -> impl ChildrenIter<(TextRange, SyntaxTrivia<'a>)> + use<'a> {
        if let Some(tok) = self.first_token() {
            Either::Right(tok.trivias_with_range())
        } else {
            Either::Left(iter::empty())
        }
    }
}

pub mod support {
    use slang::{SyntaxNode, SyntaxToken, TokenKind, ast::AstNode};

    #[inline]
    pub fn child<'a, N: AstNode<'a>>(parent: SyntaxNode<'a>) -> Option<N> {
        parent.children().filter_map(|elem| elem.as_node()).find_map(N::cast)
    }

    #[inline]
    pub fn child_token(parent: SyntaxNode, kind: TokenKind) -> Option<SyntaxToken> {
        parent.children().filter_map(|elem| elem.as_token()).find(|tok| tok.kind() == kind)
    }
}

pub trait SyntaxCursorExt {
    fn goto_first_tok_after(&mut self, offset: TextSize) -> bool;

    fn goto_first_tok_after_or_last(&mut self, offset: TextSize) -> bool;

    fn goto_last_tok_before(&mut self, offset: TextSize) -> bool;
}

impl SyntaxCursorExt for SyntaxCursor<'_> {
    fn goto_first_tok_after(&mut self, offset: TextSize) -> bool {
        let offset: usize = offset.into();
        let Some(end) = self.to_elem().range().map(|range| range.end()) else {
            return false;
        };
        if end <= offset {
            return false;
        }

        while self.to_node().is_some() {
            let success = self.goto_first_child_after_pos(offset);
            debug_assert!(success);
        }
        debug_assert!(self.to_token().is_some());
        true
    }

    fn goto_first_tok_after_or_last(&mut self, offset: TextSize) -> bool {
        if !self.goto_first_tok_after(offset) {
            if self.to_elem().range().is_some_and(|range| range.end() == usize::from(offset)) {
                while self.to_node().is_some() {
                    let success = self.goto_last_child();
                    debug_assert!(success);
                }
            } else {
                return false;
            }
        }
        true
    }

    fn goto_last_tok_before(&mut self, offset: TextSize) -> bool {
        let offset: usize = offset.into();
        let Some(start) = self.to_elem().range().map(|range| range.start()) else {
            return false;
        };
        if start >= offset {
            return false;
        }

        while self.to_node().is_some() {
            let success = self.goto_last_child_before_pos(offset);
            debug_assert!(success);
        }
        debug_assert!(self.to_token().is_some());
        true
    }
}

pub trait AstNodeExt {
    fn to_ptr(&self) -> SyntaxNodePtr;
}

impl<'a, T> AstNodeExt for T
where
    T: AstNode<'a>,
{
    #[inline]
    fn to_ptr(&self) -> SyntaxNodePtr {
        SyntaxNodePtr::from_node(self.syntax())
    }
}
