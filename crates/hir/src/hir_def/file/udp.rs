use la_arena::Idx;
use syntax::ast;

use crate::{define_src_with_name, hir_def::Ident};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpDecl {
    pub name: Option<Ident>,
}

pub type UdpDeclId = Idx<UdpDecl>;
define_src_with_name!(UdpDeclSrc(ast::UdpDeclaration));
