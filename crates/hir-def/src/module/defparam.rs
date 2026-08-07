use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use crate::{
    ast_id_map::SourceAstId,
    expr::ExprId,
    lower::{LoweringCtx, ModuleItemStore},
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

pub type DefParamSrc = SourceAstId;

impl<Store: ModuleItemStore> LoweringCtx<Store> {
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

        let source = self.source_id(defparam.syntax());
        let (defparams, sources) = self.defparams();
        crate::alloc_with_source_entry(defparams, sources, DefParam { assignments }, source)
    }
}
