use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::ast;

use crate::{
    db::InternDb,
    define_src,
    hir_def::{
        alloc_idx_and_src,
        expr::{
            Assign, Expr, ExprSrc, LowerExpr, impl_lower_expr,
            timing_control::{
                DelayControl, EventExpr, EventExprSrc, LowerEventExpr, TimingControl,
                impl_lower_event_expr,
            },
        },
        ty::{DriveStrength, lower_drive_strength},
    },
    source_map::SourceMap,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ContAssign {
    strength: Option<DriveStrength>,
    delay: Option<DelayControl>,
    assigns: SmallVec<[Assign; 1]>,
}

pub type ContAssignId = Idx<ContAssign>;
define_src!(ContAssignSrc(ast::ContinuousAssign));

pub(crate) struct LowerContAssignCtx<'a> {
    pub(crate) db: &'a dyn InternDb,

    pub(crate) cont_assigns: &'a mut Arena<ContAssign>,
    pub(crate) assign_srcs: &'a mut SourceMap<ContAssignSrc, ContAssign>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,

    pub(crate) event_exprs: &'a mut Arena<EventExpr>,
    pub(crate) event_expr_srcs: &'a mut SourceMap<EventExprSrc, EventExpr>,
}

pub(crate) trait LowerContAssign: LowerExpr + LowerEventExpr {
    fn cont_assign_ctx(&mut self) -> LowerContAssignCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_cont_assign($ctx:ty, $data:ident, $src_map:ident) {
    impl $crate::hir_def::module::continuous_assgin::LowerContAssign for $ctx {
        fn cont_assign_ctx(
            &mut self,
        ) -> $crate::hir_def::module::continuous_assgin::LowerContAssignCtx<'_> {
            $crate::hir_def::module::continuous_assgin::LowerContAssignCtx {
                db: self.db,
                cont_assigns: &mut self.$data.cont_assigns,
                assign_srcs: &mut self.$src_map.assign_srcs,
                exprs: &mut self.$data.exprs,
                expr_srcs: &mut self.$src_map.expr_srcs,
                event_exprs: &mut self.$data.event_exprs,
                event_expr_srcs: &mut self.$src_map.event_expr_srcs,
            }
        }
    }
}

impl_lower_expr!(LowerContAssignCtx<'_>);
impl_lower_event_expr!(LowerContAssignCtx<'_>);

impl LowerContAssignCtx<'_> {
    pub(crate) fn lower_continuous_assign(
        &mut self,
        assign: ast::ContinuousAssign,
    ) -> ContAssignId {
        let strength = assign.strength().map(lower_drive_strength);
        let delay = assign.delay().map(|control| {
            let control = self.event_expr_ctx().lower_timing_control(control);
            match control {
                TimingControl::DelayControl(control) => control,
                _ => unreachable!(),
            }
        });
        let assigns = assign
            .assignments()
            .children()
            .flat_map(|assign| self.expr_ctx().lower_assign(assign))
            .collect();

        let continuous_assign = ContAssign { strength, delay, assigns };
        alloc_idx_and_src! {
            continuous_assign => self.cont_assigns,
            assign => self.assign_srcs,
        }
    }
}
