use la_arena::Idx;
use syntax::ast;

use crate::{define_src_with_name, hir_def::Ident};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConfigDecl {
    pub name: Option<Ident>,
}

pub type ConfigDeclId = Idx<ConfigDecl>;
define_src_with_name!(ConfigDeclSrc(ast::ConfigDeclaration));
