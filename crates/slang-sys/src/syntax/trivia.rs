use std::{marker::PhantomData, ptr::NonNull};

use super::{
    ffi,
    range::SourceLocation,
    syntax_node::{SyntaxNode, SyntaxToken},
    tree::SyntaxTree,
};
use crate::token::TriviaKind;

/// Trivia attached to a syntax token, such as whitespace, comments, or
/// directives.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTrivia<'a> {
    pub(crate) raw: NonNull<ffi::SyntaxTrivia>,
    raw_text: String,
    pub(crate) _marker: PhantomData<&'a SyntaxTree>,
}

/// Source location for a piece of token trivia.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTriviaLoc {
    pub buffer_id: u32,
    pub start: usize,
    pub end: usize,
}

impl SyntaxTrivia<'_> {
    pub(crate) fn from_raw(raw: *const ffi::SyntaxTrivia) -> Self {
        let raw = NonNull::new(raw.cast_mut()).expect("slang returned null trivia pointer");
        let raw_text = unsafe { ffi::syntax_trivia_raw_text(raw.as_ptr()) };
        Self { raw, raw_text, _marker: PhantomData }
    }

    pub fn kind(&self) -> TriviaKind {
        TriviaKind::from_raw(unsafe { ffi::syntax_trivia_kind(self.raw.as_ptr()) })
    }

    pub fn get_raw_text(&self) -> &str {
        &self.raw_text
    }

    pub(crate) fn explicit_location(&self) -> Option<SourceLocation> {
        let valid = unsafe { ffi::syntax_trivia_explicit_location_valid(self.raw.as_ptr()) };
        valid.then(|| SourceLocation {
            buffer_id: unsafe { ffi::syntax_trivia_explicit_location_buffer_id(self.raw.as_ptr()) },
            offset: unsafe { ffi::syntax_trivia_explicit_location_offset(self.raw.as_ptr()) },
        })
    }

    pub fn syntax(&self) -> Option<SyntaxNode<'_>> {
        SyntaxNode::from_nullable_raw(unsafe { ffi::syntax_trivia_syntax(self.raw.as_ptr()) })
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
