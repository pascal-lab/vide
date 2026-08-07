use la_arena::Idx;

use crate::Ident;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConfigDecl {
    pub name: Option<Ident>,
}

pub type ConfigDeclId = Idx<ConfigDecl>;
