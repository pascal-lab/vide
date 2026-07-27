use std::marker::PhantomData;

use super::{syntax_node::SyntaxNode, tree::SyntaxTree};
use crate::token::TriviaKind;

/// Trivia attached to a syntax token, such as whitespace, comments, or
/// directives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTrivia<'a> {
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
    pub fn kind(&self) -> TriviaKind {
        unimplemented!("SyntaxTrivia::kind is not wired to slang trivia yet")
    }

    pub fn get_raw_text(&self) -> &str {
        unimplemented!("SyntaxTrivia::get_raw_text is not wired to slang trivia yet")
    }

    pub fn syntax(&self) -> Option<SyntaxNode<'_>> {
        unimplemented!("SyntaxTrivia::syntax is not wired to slang trivia yet")
    }
}
