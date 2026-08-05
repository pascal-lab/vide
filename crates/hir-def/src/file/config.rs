use la_arena::Idx;
use syntax::ast;

use crate::{
    Ident,
    source_map::{AstKind, NamedAstId},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConfigDecl {
    pub name: Option<Ident>,
}

pub type ConfigDeclId = Idx<ConfigDecl>;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct ConfigDeclarationAst;

impl AstKind for ConfigDeclarationAst {
    type Node<'a> = ast::ConfigDeclaration<'a>;
}

pub type ConfigDeclSrc = NamedAstId<ConfigDeclarationAst>;
