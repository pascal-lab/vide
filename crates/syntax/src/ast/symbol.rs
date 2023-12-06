mod generated;

use crate::ast::{AstNode, SyntaxNode};

pub use generated::*;

pub struct ErrorNode<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> AstNode<'a> for ErrorNode<'a> {
    fn can_cast(syntax: SyntaxNode<'a>) -> bool {
        syntax.is_error()
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax).then_some(ErrorNode { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

pub struct MissingNode<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> AstNode<'a> for MissingNode<'a> {
    fn can_cast(syntax: SyntaxNode<'a>) -> bool {
        syntax.is_missing()
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax).then_some(MissingNode { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}

pub struct Comment<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> AstNode<'a> for Comment<'a> {
    fn can_cast(syntax: SyntaxNode<'a>) -> bool {
        syntax.is_extra()
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax).then_some(Comment { syntax })
    }

    fn syntax(&self) -> &SyntaxNode {
        &self.syntax
    }
}
