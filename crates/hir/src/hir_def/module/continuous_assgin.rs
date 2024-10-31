use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast;

use super::LowerModuleCtx;
use crate::{
    alloc_idx_and_src, define_src,
    hir_def::{
        expr::{
            Assign, LowerExpr,
            timing_control::{DelayControl, LowerEventExpr, TimingControl},
        },
        ty::{DriveStrength, lower_drive_strength},
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ContinuousAssign {
    strength: Option<DriveStrength>,
    delay: Option<DelayControl>,
    assigns: SmallVec<[Assign; 1]>,
}

pub type ContinuousAssignId = Idx<ContinuousAssign>;
define_src!(ContinuousAssignSrc(ast::ContinuousAssign));

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_continuous_assign(
        &mut self,
        assign: ast::ContinuousAssign,
    ) -> ContinuousAssignId {
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

        let continuous_assign = ContinuousAssign { strength, delay, assigns };
        alloc_idx_and_src! {
            continuous_assign => self.module.cont_assigns,
            assign => self.module_source_map.assign_srcs,
        }
    }
}
