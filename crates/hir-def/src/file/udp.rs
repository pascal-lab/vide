use la_arena::Idx;

use crate::{Ident, ast_id_map::SourceAstId};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpDecl {
    pub name: Option<Ident>,
}

pub type UdpDeclId = Idx<UdpDecl>;

pub type UdpDeclSrc = SourceAstId;
