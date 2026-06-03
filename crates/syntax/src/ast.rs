#![allow(unused)]
#![allow(clippy::enum_variant_names)]

use std::marker::PhantomData;

use crate::{SyntaxChildren, SyntaxKind, SyntaxNode, SyntaxToken};

pub trait AstNode<'a>: Copy + Clone {
    fn can_cast(kind: SyntaxKind) -> bool
    where
        Self: Sized;

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self>
    where
        Self: Sized;

    fn syntax(&self) -> SyntaxNode<'a>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TokenList<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> TokenList<'a> {
    pub fn children(&self) -> impl Iterator<Item = SyntaxToken<'a>> + 'a {
        SyntaxChildren::new(self.syntax).filter_map(|elem| elem.as_token())
    }
}

impl<'a> AstNode<'a> for TokenList<'a> {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::TOKEN_LIST
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxList<'a, T: AstNode<'a>> {
    syntax: SyntaxNode<'a>,
    _marker: PhantomData<T>,
}

impl<'a, T: AstNode<'a> + 'a> SyntaxList<'a, T> {
    pub fn children(&self) -> impl Iterator<Item = T> + 'a {
        SyntaxChildren::new(self.syntax).filter_map(|elem| elem.as_node()).filter_map(T::cast)
    }
}

impl<'a, T: AstNode<'a>> AstNode<'a> for SyntaxList<'a, T> {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SYNTAX_LIST
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax, _marker: PhantomData })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SeparatedList<'a, T: AstNode<'a>> {
    syntax: SyntaxNode<'a>,
    _marker: PhantomData<T>,
}

impl<'a, T: AstNode<'a> + 'a> SeparatedList<'a, T> {
    pub fn children(&self) -> impl Iterator<Item = T> + 'a {
        SyntaxChildren::new(self.syntax)
            .step_by(2)
            .filter_map(|elem| elem.as_node())
            .filter_map(T::cast)
    }

    pub fn children_with_separators(&self) -> SyntaxChildren<'a> {
        SyntaxChildren::new(self.syntax)
    }
}

impl<'a, T: AstNode<'a>> AstNode<'a> for SeparatedList<'a, T> {
    fn can_cast(kind: SyntaxKind) -> bool {
        kind == SyntaxKind::SEPARATED_LIST
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Self::can_cast(syntax.kind()).then_some(Self { syntax, _marker: PhantomData })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HybridNode<'a> {
    syntax: SyntaxNode<'a>,
}

impl<'a> AstNode<'a> for HybridNode<'a> {
    fn can_cast(_: SyntaxKind) -> bool {
        true
    }

    fn cast(syntax: SyntaxNode<'a>) -> Option<Self> {
        Some(Self { syntax })
    }

    fn syntax(&self) -> SyntaxNode<'a> {
        self.syntax
    }
}

include!("generated/ast.rs");
