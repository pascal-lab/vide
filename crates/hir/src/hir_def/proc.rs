use la_arena::{Arena, Idx};
use syntax::ast;

use super::{
    expr::{declarator::impl_lower_decl, impl_lower_expr, timing_control::impl_lower_event_expr},
    stmt::impl_lower_stmt,
};
use crate::{
    container::ContainerId,
    db::InternDb,
    define_src,
    file::HirFileId,
    hir_def::{
        alloc_idx_and_src,
        expr::{
            Expr, ExprSrc,
            declarator::{Declarator, DeclaratorSrc},
            timing_control::{EventExpr, EventExprSrc},
        },
        stmt::{LowerStmt, Stmt, StmtId, StmtSrc},
    },
    source_map::SourceMap,
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
    pub stmt: StmtId,
}

pub type ProcId = Idx<Proc>;

define_src!(ProcSrc(ast::ProceduralBlock));

pub(crate) trait LowerProc: LowerStmt {
    fn proc_ctx(&mut self) -> LowerProcCtx<'_>;
}

pub(crate) struct LowerProcCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) cont_id: ContainerId,

    pub(crate) procs: &'a mut Arena<Proc>,
    pub(crate) proc_srcs: &'a mut SourceMap<ProcSrc, Proc>,

    pub(crate) stmts: &'a mut Arena<Stmt>,
    pub(crate) stmt_srcs: &'a mut SourceMap<StmtSrc, Stmt>,

    pub(crate) event_exprs: &'a mut Arena<EventExpr>,
    pub(crate) event_expr_srcs: &'a mut SourceMap<EventExprSrc, EventExpr>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,

    pub(crate) decls: &'a mut Arena<Declarator>,
    pub(crate) decl_srcs: &'a mut SourceMap<DeclaratorSrc, Declarator>,
}

impl_lower_expr!(LowerProcCtx<'_>);
impl_lower_decl!(LowerProcCtx<'_>);
impl_lower_event_expr!(LowerProcCtx<'_>);
impl_lower_stmt!(LowerProcCtx<'_>, cont_id);

impl LowerProcCtx<'_> {
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

        let stmt = self.stmt_ctx().lower_stmt(proc.statement());

        alloc_idx_and_src! {
            Proc { proc_ty, stmt } => self.procs,
            proc => self.proc_srcs,
        }
    }
}
