use la_arena::Idx;

use crate::{Ident, ast_id_map::SourceAstId};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConfigDecl {
    pub name: Option<Ident>,
}

pub type ConfigDeclId = Idx<ConfigDecl>;

pub type ConfigDeclSrc = SourceAstId;
