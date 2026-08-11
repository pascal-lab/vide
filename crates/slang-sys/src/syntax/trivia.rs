use std::{
    hash::{Hash, Hasher},
    ptr::NonNull,
};

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
    pub(crate) raw: NonNull<ffi::SyntaxTrivia>,
    raw_text: String,
    pub(crate) tree: &'a SyntaxTree,
}

impl PartialEq for SyntaxTrivia<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.raw_text == other.raw_text
    }
}

impl Eq for SyntaxTrivia<'_> {}

impl Hash for SyntaxTrivia<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
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
    pub(crate) fn from_raw(raw: *const ffi::SyntaxTrivia, tree: &'a SyntaxTree) -> Self {
        let raw = NonNull::new(raw.cast_mut()).expect("slang returned null trivia pointer");
        let raw_text = unsafe { ffi::syntax_trivia_raw_text(raw.as_ptr()) };
        Self { raw, raw_text, tree }
    }

    pub fn kind(&self) -> TriviaKind {
        let kind = TriviaKind::from_raw(unsafe { ffi::syntax_trivia_kind(self.raw.as_ptr()) });
        if !kind.is_unknown() {
            return kind;
        }

        // Slang can expose an implicit source trivia with an Unknown kind
        // while retaining its raw text. Preserve the useful lexical contract
        // at this boundary instead of making every consumer special-case it.
        let raw = self.get_raw_text();
        if !raw.is_empty() && raw.bytes().any(|byte| matches!(byte, b'\r' | b'\n')) {
            return TriviaKind::END_OF_LINE;
        }
        if !raw.is_empty() && raw.chars().all(char::is_whitespace) {
            return TriviaKind::WHITESPACE;
        }
        kind
    }

    pub fn get_raw_text(&self) -> &str {
        &self.raw_text
    }

    pub(crate) fn explicit_location(&self) -> Option<SourceLocation> {
        let valid = unsafe { ffi::syntax_trivia_explicit_location_valid(self.raw.as_ptr()) };
        valid.then(|| {
            SourceLocation::from_parts(
                unsafe { ffi::syntax_trivia_explicit_location_buffer_id(self.raw.as_ptr()) },
                unsafe { ffi::syntax_trivia_explicit_location_offset(self.raw.as_ptr()) },
            )
        })
    }

    pub fn syntax(&self) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_nullable_raw(
            unsafe { ffi::syntax_trivia_syntax(self.raw.as_ptr()) },
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
