use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use crate::{
    db::InternDb,
    define_src,
    hir_def::{
        alloc_idx_and_src,
        expr::{Expr, ExprId, ExprSrc, LowerExpr, impl_lower_expr},
    },
    source_map::SourceMap,
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

pub(crate) struct LowerDefParamCtx<'a> {
    pub(crate) db: &'a dyn InternDb,

    pub(crate) defparams: &'a mut Arena<DefParam>,
    pub(crate) defparam_srcs: &'a mut SourceMap<DefParamSrc, DefParam>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,
}

pub(crate) trait LowerDefParam: LowerExpr {
    fn defparam_ctx(&mut self) -> LowerDefParamCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_defparam($ctx:ty, $data:ident, $src_map:ident) {
    impl $crate::hir_def::module::defparam::LowerDefParam for $ctx {
        fn defparam_ctx(&mut self) -> $crate::hir_def::module::defparam::LowerDefParamCtx<'_> {
            $crate::hir_def::module::defparam::LowerDefParamCtx {
                db: self.db,
                defparams: &mut self.$data.defparams,
                defparam_srcs: &mut self.$src_map.defparam_srcs,
                exprs: &mut self.$data.exprs,
                expr_srcs: &mut self.$src_map.expr_srcs,
            }
        }
    }
}

impl_lower_expr!(LowerDefParamCtx<'_>);

impl LowerDefParamCtx<'_> {
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
            DefParam { assignments } => self.defparams,
            defparam => self.defparam_srcs,
        }
    }
}
