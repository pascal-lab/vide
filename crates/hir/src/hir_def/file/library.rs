use la_arena::Idx;
use syntax::ast;

use crate::{
    hir_def::Ident,
    source_map::{AstId, AstKind, NamedAstId},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryDecl {
    pub name: Option<Ident>,
}

pub type LibraryDeclId = Idx<LibraryDecl>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct LibraryDeclarationAst;

impl AstKind for LibraryDeclarationAst {
    type Node<'a> = ast::LibraryDeclaration<'a>;
}

pub type LibraryDeclSrc = NamedAstId<LibraryDeclarationAst>;

impl From<ast::LibraryDeclaration<'_>> for LibraryDeclSrc {
    fn from(library: ast::LibraryDeclaration<'_>) -> Self {
        Self::from_ast(library)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryInclude;

pub type LibraryIncludeId = Idx<LibraryInclude>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct LibraryIncludeStatementAst;

impl AstKind for LibraryIncludeStatementAst {
    type Node<'a> = ast::LibraryIncludeStatement<'a>;
}

pub type LibraryIncludeSrc = AstId<LibraryIncludeStatementAst>;

impl From<ast::LibraryIncludeStatement<'_>> for LibraryIncludeSrc {
    fn from(include: ast::LibraryIncludeStatement<'_>) -> Self {
        Self::from_ast(include)
    }
}
