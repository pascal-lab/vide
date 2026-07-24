use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast;

use crate::{
    hir_def::{
        alloc_with_source,
        expr::{
            Assign,
            timing_control::{DelayControl, TimingControl},
        },
        lower::{LoweringCtx, ModuleItemStore},
        ty::{DriveStrength, lower_drive_strength},
    },
    source_map::{AstId, AstKind},
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ContAssign {
    strength: Option<DriveStrength>,
    delay: Option<DelayControl>,
    assigns: SmallVec<[Assign; 1]>,
}

pub type ContAssignId = Idx<ContAssign>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ContinuousAssignAst;

impl AstKind for ContinuousAssignAst {
    type Node<'a> = ast::ContinuousAssign<'a>;
}

pub type ContAssignSrc = AstId<ContinuousAssignAst>;

impl<Store: ModuleItemStore> LoweringCtx<'_, Store> {
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
        let file_id = self.file_id;
        let (continuous_assigns, sources) = self.continuous_assigns();
        alloc_with_source(file_id, continuous_assigns, sources, continuous_assign, assign)
    }
}
