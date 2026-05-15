use std::iter;

use either::Either;
use slang::{
    ChildrenIter, SyntaxAncestors, SyntaxCursor, SyntaxElement, SyntaxNode, SyntaxTokenWithParent,
    SyntaxTrivia, TokenKind, Trivia, TriviaKind, ast::AstNode,
};
use token::SyntaxTokenExt;
use utils::line_index::{TextRange, TextSize};

use crate::{has_text_range::HasTextRange, ptr::SyntaxNodePtr};

pub mod ast_ext;
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
    fn token_at_offset_including_eof(&self, offset: TextSize) -> TokenAtOffset<'a>;
    fn token_after_or_at_offset(&self, offset: TextSize) -> Option<SyntaxTokenWithParent<'a>>;
    fn token_before_offset(&self, offset: TextSize) -> Option<SyntaxTokenWithParent<'a>>;
    fn trivia_kind_at_offset(&self, offset: TextSize) -> Option<TriviaKind>;
    fn find_node_at_offset<N: AstNode<'a>>(&self, offset: TextSize) -> Option<N>;
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
        let Some(range) = self.text_range() else {
            return TokenAtOffset::None;
        };
        if range.is_empty() || !(range.contains(offset)) {
            return TokenAtOffset::None;
        }

        let mut cursor = self.walk();
        cursor.goto_last_tok_before(offset);
        let left = cursor.to_tok_with_parent();
        let left_range = left.and_then(|left| left.text_range());
        if left_range.is_some_and(|range| range.contains(offset))
            && let Some(left) = left
        {
            return TokenAtOffset::Single(left);
        }
        let left_ok = left_range.map(|range| range.end() == offset).unwrap_or(false);

        cursor.reset_to_root();
        cursor.goto_first_tok_after(offset);
        let right = cursor.to_tok_with_parent();
        let right_range = right.and_then(|right| right.text_range());
        let right_ok = right_range.map(|range| range.contains(offset)).unwrap_or(false);

        match (left_ok, right_ok) {
            (true, true) => match (left, right) {
                (Some(left), Some(right)) => TokenAtOffset::Between(left, right),
                _ => TokenAtOffset::None,
            },
            (true, false) => left.map_or(TokenAtOffset::None, TokenAtOffset::Single),
            (false, true) => right.map_or(TokenAtOffset::None, TokenAtOffset::Single),
            (false, false) => TokenAtOffset::None,
        }
    }

    fn token_at_offset_including_eof(&self, offset: TextSize) -> TokenAtOffset<'a> {
        let at = self.token_at_offset(offset);
        if !matches!(at, TokenAtOffset::None) {
            return at;
        }

        let Some(range) = self.text_range() else {
            return TokenAtOffset::None;
        };
        if offset != range.end() {
            return TokenAtOffset::None;
        }

        self.token_before_offset(offset)
            .and_then(|tok| tok.text_range().is_some_and(|r| r.end() == offset).then_some(tok))
            .map_or(TokenAtOffset::None, TokenAtOffset::Single)
    }

    fn token_after_or_at_offset(&self, offset: TextSize) -> Option<SyntaxTokenWithParent<'a>> {
        if let Some(tok) = self.token_at_offset(offset).left_biased()
            && tok.text_range().is_some_and(|r| r.contains(offset))
        {
            return Some(tok);
        }

        let mut cursor = self.walk();
        if !cursor.goto_first_tok_after_or_last(offset) {
            return None;
        }
        cursor.to_tok_with_parent()
    }

    fn token_before_offset(&self, offset: TextSize) -> Option<SyntaxTokenWithParent<'a>> {
        let mut cursor = self.walk();
        if !cursor.goto_last_tok_before(offset) {
            return None;
        }
        cursor.to_tok_with_parent()
    }

    fn trivia_kind_at_offset(&self, offset: TextSize) -> Option<TriviaKind> {
        fn trivia_kind_at_offset_in_token(
            tok: SyntaxTokenWithParent<'_>,
            offset: TextSize,
        ) -> Option<TriviaKind> {
            for ((start, end), trivia) in tok.tok.trivias_with_loc() {
                let range = TextRange::new(TextSize::new(start as u32), TextSize::new(end as u32));
                if range.contains(offset) {
                    return Some(trivia.kind());
                }

                // For directive trivia, check nested trivia in the directive's first token.
                if trivia.kind() == Trivia!["`"]
                    && let Some(node) = trivia.syntax()
                    && let Some(first_tok) = node.first_token()
                {
                    for ((ns, ne), nested_trivia) in first_tok.trivias_with_loc() {
                        let nested_range =
                            TextRange::new(TextSize::new(ns as u32), TextSize::new(ne as u32));
                        if nested_range.contains(offset) {
                            return Some(nested_trivia.kind());
                        }
                    }
                }
            }

            None
        }

        // Trivia can be attached to either the token before it or after it, depending
        // on how the underlying parser decides to associate it. Check both
        // directions, plus the last token for trivia-only files.
        if let Some(tok) = self.token_after_or_at_offset(offset)
            && let Some(kind) = trivia_kind_at_offset_in_token(tok, offset)
        {
            return Some(kind);
        }

        if let Some(tok) = self.token_before_offset(offset)
            && let Some(kind) = trivia_kind_at_offset_in_token(tok, offset)
        {
            return Some(kind);
        }

        let mut cursor = self.walk();
        let end = self.text_range()?.end();
        if !cursor.goto_first_tok_after_or_last(end) {
            return None;
        }
        let last = cursor.to_tok_with_parent()?;
        trivia_kind_at_offset_in_token(last, offset)
    }

    fn find_node_at_offset<N: AstNode<'a>>(&self, offset: TextSize) -> Option<N> {
        fn best_match_in_ancestors<'a, N: AstNode<'a>>(
            start: SyntaxNode<'a>,
        ) -> Option<(TextSize, N)> {
            let mut best: Option<(TextSize, N)> = None;
            for anc in SyntaxAncestors::start_from(start) {
                let Some(cast) = N::cast(anc) else {
                    continue;
                };
                let len = anc
                    .text_range()
                    .map(|range| range.end() - range.start())
                    .unwrap_or_else(|| TextSize::from(u32::MAX));
                match &best {
                    Some((best_len, _)) if *best_len <= len => {}
                    _ => best = Some((len, cast)),
                }
            }
            best
        }

        let elem = self.covering_element(TextRange::empty(offset));
        let mut seeds: Vec<SyntaxNode<'a>> =
            elem.as_node().or_else(|| elem.parent()).into_iter().collect();
        if let Some(prev) = self.token_before_offset(offset) {
            seeds.push(prev.parent);
        }
        if let Some(next) = self.token_after_or_at_offset(offset) {
            seeds.push(next.parent);
        }

        let mut best: Option<(TextSize, N)> = None;
        for seed in seeds {
            let Some((len, node)) = best_match_in_ancestors::<N>(seed) else {
                continue;
            };
            match &best {
                Some((best_len, _)) if *best_len <= len => {}
                _ => best = Some((len, node)),
            }
        }

        best.map(|(_, node)| node)
    }

    fn token_or_node_at_offset(
        &self,
        offset: TextSize,
    ) -> Either<TokenAtOffset<'a>, SyntaxNode<'a>> {
        let Some(range) = self.text_range() else {
            return Either::Left(TokenAtOffset::None);
        };
        if range.is_empty() || !(range.contains(offset)) {
            return Either::Left(TokenAtOffset::None);
        }

        let mut cursor = self.walk();
        cursor.goto_last_tok_before(offset);
        let left = cursor.to_tok_with_parent();
        let left_range = left.and_then(|left| left.text_range());
        if left_range.is_some_and(|range| range.contains(offset))
            && let Some(left) = left
        {
            return Either::Left(TokenAtOffset::Single(left));
        }
        let left_ok = left_range.map(|range| range.end() == offset).unwrap_or(false);

        cursor.reset_to_root();
        cursor.goto_first_tok_after(offset);
        let right = cursor.to_tok_with_parent();
        let right_range = right.and_then(|right| right.text_range());
        let right_ok = right_range.map(|range| range.contains(offset)).unwrap_or(false);

        match (left_ok, right_ok) {
            (true, true) => match (left, right) {
                (Some(left), Some(right)) => Either::Left(TokenAtOffset::Between(left, right)),
                _ => Either::Left(TokenAtOffset::None),
            },
            (true, false) => Either::Left(left.map_or(TokenAtOffset::None, TokenAtOffset::Single)),
            (false, true) => Either::Left(right.map_or(TokenAtOffset::None, TokenAtOffset::Single)),
            (false, false) => {
                while !cursor.to_elem().text_range().is_some_and(|range| range.contains(offset)) {
                    if !cursor.goto_parent() {
                        return Either::Left(TokenAtOffset::None);
                    }
                }
                if let Some(node) = cursor.to_node() {
                    Either::Right(node)
                } else {
                    Either::Left(TokenAtOffset::None)
                }
            }
        }
    }

    #[inline]
    fn find_root(&self) -> SyntaxNode<'a> {
        SyntaxAncestors::start_from(*self).last().unwrap_or(*self)
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
