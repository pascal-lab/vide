mod support;
mod symbol;

pub use symbol::*;

use crate::SyntaxNode;

pub trait AstNode<'a> {
    fn can_cast(syntax: SyntaxNode<'a>) -> bool
    where
        Self: Sized;

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self>
    where
        Self: Sized;

    fn syntax(&self) -> &'a SyntaxNode;

    fn errors(&'a self) -> support::AstChildren<'a, symbol::ErrorNode<'a>> {
        support::children(self.syntax())
    }

    fn comments(&'a self) -> support::AstChildren<'a, symbol::Comment<'a>> {
        support::children(self.syntax())
    }
}
