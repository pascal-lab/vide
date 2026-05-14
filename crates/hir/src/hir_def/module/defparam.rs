use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use super::LowerModuleCtx;
use crate::{
    define_src,
    hir_def::{
        alloc_idx_and_src,
        expr::{ExprId, LowerExpr},
    },
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
define_src!(DefParamSrc(ast::DefParam));

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_defparam(&mut self, defparam: ast::DefParam) -> DefParamId {
        let assignments = defparam
            .assignments()
            .children()
            .map(|assignment| {
                let target = self
                    .expr_ctx()
                    .lower_expr_opt(ast::Expression::cast(assignment.name().syntax()));
                let value = self.expr_ctx().lower_expr(assignment.setter().expr());
                DefParamAssignment { target, value }
            })
            .collect();

        alloc_idx_and_src! {
            DefParam { assignments } => self.module.defparams,
            defparam => self.module_source_map.defparam_srcs,
        }
    }
}
