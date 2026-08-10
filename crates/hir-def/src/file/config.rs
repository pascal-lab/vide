use la_arena::Idx;
use syntax::SyntaxKind;

use crate::Ident;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConfigDecl {
    pub name: Option<Ident>,
    pub top_cells: Box<[Ident]>,
    pub rules: Box<[ConfigRule]>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConfigRule {
    pub kind: SyntaxKind,
}

pub type ConfigDeclId = Idx<ConfigDecl>;
