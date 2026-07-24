use la_arena::Idx;
use syntax::ast;

use super::{
    alloc_with_source,
    lower::{LoweringCtx, ProcStore},
    stmt::StmtId,
};
use crate::source_map::{AstId, AstKind};

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
    pub stmt: StmtId,
}

pub type ProcId = Idx<Proc>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ProceduralBlockAst;

impl AstKind for ProceduralBlockAst {
    type Node<'a> = ast::ProceduralBlock<'a>;
}

pub type ProcSrc = AstId<ProceduralBlockAst>;

impl<Store: ProcStore> LoweringCtx<'_, Store> {
    pub(crate) fn lower_proc(&mut self, proc: ast::ProceduralBlock) -> ProcId {
        use ast::ProceduralBlock::*;
        let proc_ty = match proc {
            AlwaysFFBlock(_) => ProcType::Always(AlwaysKeyword::AlwaysFf),
            AlwaysBlock(_) => ProcType::Always(AlwaysKeyword::Always),
            AlwaysCombBlock(_) => ProcType::Always(AlwaysKeyword::AlwaysComb),
            AlwaysLatchBlock(_) => ProcType::Always(AlwaysKeyword::AlwaysLatch),
            InitialBlock(_) => ProcType::Initial,
            FinalBlock(_) => ProcType::Final,
        };

        let stmt = self.lower_stmt(proc.statement());

        let file_id = self.file_id;
        let (procs, sources) = self.procs();
        alloc_with_source(file_id, procs, sources, Proc { proc_ty, stmt }, proc)
    }
}
