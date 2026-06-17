use la_arena::Idx;
use syntax::ast;

use crate::{
    hir_def::Ident,
    source_map::{AstKind, NamedAstId},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpDecl {
    pub name: Option<Ident>,
}

pub type UdpDeclId = Idx<UdpDecl>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct UdpDeclarationAst;

impl AstKind for UdpDeclarationAst {
    type Node<'a> = ast::UdpDeclaration<'a>;
}

pub type UdpDeclSrc = NamedAstId<UdpDeclarationAst>;

impl From<ast::UdpDeclaration<'_>> for UdpDeclSrc {
    fn from(udp: ast::UdpDeclaration<'_>) -> Self {
        Self::from_ast(udp)
    }
}
