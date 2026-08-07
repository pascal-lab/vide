use la_arena::Idx;

use crate::Ident;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpDecl {
    pub name: Option<Ident>,
}

pub type UdpDeclId = Idx<UdpDecl>;
