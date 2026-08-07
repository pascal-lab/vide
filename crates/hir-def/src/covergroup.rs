use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast;

use crate::{Ident, lower_ident_opt, lower_named_label_opt};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CovergroupDef {
    pub name: Option<Ident>,
    pub coverpoints: SmallVec<[CoverpointId; 4]>,
    pub crosses: SmallVec<[CrossId; 2]>,
}

pub type CovergroupId = Idx<CovergroupDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CoverpointDef {
    pub name: Option<Ident>,
}

pub type CoverpointId = Idx<CoverpointDef>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CrossDef {
    pub name: Option<Ident>,
}

pub type CrossId = Idx<CrossDef>;

pub fn lower_covergroup_decl(covergroup: ast::CovergroupDeclaration<'_>) -> CovergroupDef {
    CovergroupDef {
        name: lower_ident_opt(covergroup.name()),
        coverpoints: SmallVec::new(),
        crosses: SmallVec::new(),
    }
}

pub fn lower_coverpoint(coverpoint: ast::Coverpoint<'_>) -> CoverpointDef {
    CoverpointDef { name: lower_named_label_opt(coverpoint.label()) }
}

pub fn lower_cross(cross: ast::CoverCross<'_>) -> CrossDef {
    CrossDef { name: lower_named_label_opt(cross.label()) }
}
