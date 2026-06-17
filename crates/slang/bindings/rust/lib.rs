#![feature(trait_alias)]

pub mod ast;
mod diagnostic;
mod facts;
mod ffi;
mod preprocessor_trace;
mod source_buffer;
mod syntax;
mod syntax_tree;
mod token;
mod value;

pub use diagnostic::*;
pub use facts::*;
pub use ffi::CxxSV;
pub use preprocessor_trace::*;
pub use source_buffer::*;
pub use syntax::{
    SyntaxKind, TokenKind, TriviaKind,
    cursor::SyntaxCursor,
    iter::{
        SyntaxAncestors, SyntaxChildren, SyntaxElemPreorder, SyntaxIdxChildren, SyntaxNodePreorder,
        WalkEvent,
    },
};
pub use syntax_tree::*;
pub use value::*;

#[cfg(test)]
mod tests;
