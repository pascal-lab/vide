use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};

use crate::{
    expr::{
        Assign,
        timing_control::{DelayControl, TimingControl},
    },
    lower::{LoweringCtx, ModuleItemStore},
    ty::{DriveStrength, lower_drive_strength},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ContAssign {
    strength: Option<DriveStrength>,
    delay: Option<DelayControl>,
    assigns: SmallVec<[Assign; 1]>,
}

pub type ContAssignId = Idx<ContAssign>;

impl<Store: ModuleItemStore> LoweringCtx<Store> {
    pub(crate) fn lower_continuous_assign(
        &mut self,
        assign: ast::ContinuousAssign,
    ) -> ContAssignId {
        let strength = assign.strength().map(lower_drive_strength);
        let delay = assign.delay().and_then(|control| {
            let control = self.lower_timing_control(control);
            match control {
                TimingControl::DelayControl(control) => Some(control),
                _ => None,
            }
        });
        let assigns =
            assign.assignments().children().flat_map(|assign| self.lower_assign(assign)).collect();

        let continuous_assign = ContAssign { strength, delay, assigns };
        let source = self.source_id(assign.syntax());
        let (continuous_assigns, sources) = self.continuous_assigns();
        crate::alloc_with_source_entry(continuous_assigns, sources, continuous_assign, source)
    }
}
