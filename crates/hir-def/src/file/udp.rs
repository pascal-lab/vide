use la_arena::Idx;
use smallvec::SmallVec;
use syntax::SyntaxKind;

use crate::Ident;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpDecl {
    pub name: Option<Ident>,
    pub ports: SmallVec<[Ident; 4]>,
    pub initial: Option<UdpInitialValue>,
    pub entries: SmallVec<[UdpEntry; 8]>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpInitialValue {
    pub name: Option<Ident>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct UdpEntry {
    pub input_kinds: SmallVec<[SyntaxKind; 4]>,
    pub current: Option<SyntaxKind>,
    pub next: Option<SyntaxKind>,
}

pub type UdpDeclId = Idx<UdpDecl>;
