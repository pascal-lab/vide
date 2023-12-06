use crate::{ast::AstNode, SyntaxChildren, SyntaxNode};
use std::marker::PhantomData;

pub struct AstChildren<'a, N> {
    syntax_children: SyntaxChildren<'a>,
    ph: PhantomData<N>,
}

impl<'a, N: AstNode<'a>> AstChildren<'a, N> {
    pub fn new(parent: &'a SyntaxNode) -> Self {
        AstChildren { syntax_children: SyntaxChildren::new(parent), ph: PhantomData }
    }
}

impl<'a, N: AstNode<'a>> Iterator for AstChildren<'a, N> {
    type Item = N;
    fn next(&mut self) -> Option<N> {
        self.syntax_children.find_map(N::cast)
    }
}

pub(crate) fn child<'a, N: AstNode<'a>>(parent: &'a SyntaxNode) -> Option<N> {
    SyntaxChildren::new(parent).find_map(N::cast)
}

pub(crate) fn children<'a, N: AstNode<'a>>(parent: &'a SyntaxNode) -> AstChildren<'a, N> {
    AstChildren::new(parent)
}

pub(crate) fn token<'a>(parent: &'a SyntaxNode, kind: &str) -> Option<SyntaxNode<'a>> {
    SyntaxChildren::new(parent).find(|it| it.kind() == kind)
}
