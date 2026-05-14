use la_arena::Idx;
use syntax::ast;

use crate::{define_src, define_src_with_name, hir_def::Ident};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryDecl {
    pub name: Option<Ident>,
}

pub type LibraryDeclId = Idx<LibraryDecl>;
define_src_with_name!(LibraryDeclSrc(ast::LibraryDeclaration));

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct LibraryInclude;

pub type LibraryIncludeId = Idx<LibraryInclude>;
define_src!(LibraryIncludeSrc(ast::LibraryIncludeStatement));
