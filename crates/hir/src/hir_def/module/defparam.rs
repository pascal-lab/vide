use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use crate::{
    hir_def::{
        alloc_with_source,
        expr::ExprId,
        lower::{LoweringCtx, ModuleItemStore},
    },
    source_map::{AstId, AstKind},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DefParam {
    pub assignments: SmallVec<[DefParamAssignment; 1]>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DefParamAssignment {
    pub target: ExprId,
    pub value: ExprId,
}

pub type DefParamId = Idx<DefParam>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DefParamAst;

impl AstKind for DefParamAst {
    type Node<'a> = ast::DefParam<'a>;
}

pub type DefParamSrc = AstId<DefParamAst>;

impl<Store: ModuleItemStore> LoweringCtx<'_, Store> {
    pub(crate) fn lower_defparam(&mut self, defparam: ast::DefParam) -> DefParamId {
        let assignments = defparam
            .assignments()
            .children()
            .map(|assignment| {
                let target = self.lower_expr_opt(ast::Expression::cast(assignment.name().syntax()));
                let value = self.lower_expr(assignment.setter().expr());
                DefParamAssignment { target, value }
            })
            .collect();

        let file_id = self.file_id;
        let (defparams, sources) = self.defparams();
        alloc_with_source(file_id, defparams, sources, DefParam { assignments }, defparam)
    }
}
