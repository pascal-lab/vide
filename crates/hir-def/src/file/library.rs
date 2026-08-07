use la_arena::Idx;

use crate::Ident;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryDecl {
    pub name: Option<Ident>,
}

pub type LibraryDeclId = Idx<LibraryDecl>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryInclude;

pub type LibraryIncludeId = Idx<LibraryInclude>;
