use std::hash::{Hash, Hasher};

use super::{
    ffi,
    syntax_node::{SyntaxNode, SyntaxToken},
    tree::SyntaxTree,
};
use crate::{source_buffer::SourceLocation, token::TriviaKind};

/// Trivia attached to a syntax token, such as whitespace, comments, or
/// directives.
#[derive(Clone, Debug)]
pub struct SyntaxTrivia<'a> {
    pub(crate) token: SyntaxToken<'a>,
    pub(crate) index: usize,
    raw_text: String,
    pub(crate) tree: &'a SyntaxTree,
}

impl PartialEq for SyntaxTrivia<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.token == other.token && self.index == other.index && self.raw_text == other.raw_text
    }
}

impl Eq for SyntaxTrivia<'_> {}

impl Hash for SyntaxTrivia<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.token.hash(state);
        self.index.hash(state);
        self.raw_text.hash(state);
    }
}

/// Source location for a piece of token trivia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTriviaLoc {
    pub buffer_id: u32,
    pub start: usize,
    pub end: usize,
}

impl<'a> SyntaxTrivia<'a> {
    pub(crate) fn from_token(token: SyntaxToken<'a>, index: usize) -> Self {
        assert!(index < token.trivia_count(), "trivia index should be in bounds");
        let raw_text = unsafe { ffi::syntax_trivia_raw_text(token.raw.as_ptr(), index) };
        Self { token, index, raw_text, tree: token.tree }
    }

    pub fn kind(&self) -> TriviaKind {
        TriviaKind::from_raw(unsafe {
            ffi::syntax_trivia_kind(self.token.raw.as_ptr(), self.index)
        })
    }

    pub fn get_raw_text(&self) -> &str {
        &self.raw_text
    }

    pub(crate) fn explicit_location(&self) -> Option<SourceLocation> {
        let valid = unsafe {
            ffi::syntax_trivia_explicit_location_valid(self.token.raw.as_ptr(), self.index)
        };
        valid.then(|| {
            SourceLocation::from_parts(
                unsafe {
                    ffi::syntax_trivia_explicit_location_buffer_id(
                        self.token.raw.as_ptr(),
                        self.index,
                    )
                },
                unsafe {
                    ffi::syntax_trivia_explicit_location_offset(self.token.raw.as_ptr(), self.index)
                },
            )
        })
    }

    pub fn syntax(&self) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_nullable_raw(
            unsafe { ffi::syntax_trivia_syntax(self.token.raw.as_ptr(), self.index) },
            self.tree,
        )
    }
}

/// Iterator over trivia attached to a token.
#[derive(Clone, Debug)]
pub struct SyntaxTriviaIter<'a> {
    pub(crate) tok: SyntaxToken<'a>,
    pub(crate) idx: usize,
    pub(crate) total: usize,
}

impl<'a> SyntaxTriviaIter<'a> {
    pub(crate) fn new(tok: SyntaxToken<'a>) -> Self {
        Self { tok, idx: 0, total: tok.trivia_count() }
    }
}

impl<'a> Iterator for SyntaxTriviaIter<'a> {
    type Item = SyntaxTrivia<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.total {
            return None;
        }

        let trivia =
            self.tok.trivia_at(self.idx).expect("trivia iterator index should be in bounds");
        self.idx += 1;
        Some(trivia)
    }
}

impl<'a> DoubleEndedIterator for SyntaxTriviaIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.idx >= self.total {
            return None;
        }

        self.total -= 1;
        Some(self.tok.trivia_at(self.total).expect("trivia iterator index should be in bounds"))
    }
}

impl ExactSizeIterator for SyntaxTriviaIter<'_> {
    fn len(&self) -> usize {
        self.total - self.idx
    }
}
