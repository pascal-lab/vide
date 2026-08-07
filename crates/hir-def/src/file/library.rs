use la_arena::Idx;

use crate::{Ident, ast_id_map::SourceAstId};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryDecl {
    pub name: Option<Ident>,
}

pub type LibraryDeclId = Idx<LibraryDecl>;

pub type LibraryDeclSrc = SourceAstId;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryInclude;

pub type LibraryIncludeId = Idx<LibraryInclude>;

pub type LibraryIncludeSrc = SourceAstId;
