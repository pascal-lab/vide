use la_arena::Idx;
use syntax::ast::{self, AstNode};

use super::{
    alloc_with_source,
    lower::{LoweringCtx, ProcStore},
};
use crate::{
    ast_id_map::SourceAstId,
    owner::{OwnerId, OwnerKind},
};
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum AlwaysKeyword {
    Always,
    AlwaysComb,
    AlwaysLatch,
    AlwaysFf,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ProcType {
    #[default]
    Initial,

    Always(AlwaysKeyword),
    Final,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Proc {
    pub proc_ty: ProcType,
    pub owner: OwnerId,
}

pub type ProcId = Idx<Proc>;

pub type ProcSrc = SourceAstId;

impl<Store: ProcStore> LoweringCtx<Store> {
    pub(crate) fn lower_proc(&mut self, proc: ast::ProceduralBlock) -> ProcId {
        let owner = self
            .owner_for_node(proc.syntax(), OwnerKind::ProceduralBlock)
            .expect("procedural block must have a canonical owner");
        use ast::ProceduralBlock::*;
        let proc_ty = match proc {
            AlwaysFFBlock(_) => ProcType::Always(AlwaysKeyword::AlwaysFf),
            AlwaysBlock(_) => ProcType::Always(AlwaysKeyword::Always),
            AlwaysCombBlock(_) => ProcType::Always(AlwaysKeyword::AlwaysComb),
            AlwaysLatchBlock(_) => ProcType::Always(AlwaysKeyword::AlwaysLatch),
            InitialBlock(_) => ProcType::Initial,
            FinalBlock(_) => ProcType::Final,
        };

        let source = self.source_id(proc.syntax());
        let (procs, sources) = self.procs();
        crate::alloc_with_source_entry(procs, sources, Proc { proc_ty, owner }, source)
    }
}
