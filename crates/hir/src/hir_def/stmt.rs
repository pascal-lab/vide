use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, SyntaxToken, TokenKind,
    ast::{self, AstNode},
};

use super::{
    HirData, Ident,
    block::{BlockInfo, BlockLoc},
    expr::{
        Expr, ExprId, ExprSrc, LowerExpr,
        data_ty::DataTy,
        declarator::{DeclId, Declarator, DeclaratorSrc, LowerDecl, impl_lower_decl},
        impl_lower_expr,
        timing_control::{
            EventExpr, EventExprSrc, LowerEventExpr, TimingControl, impl_lower_event_expr,
        },
    },
    lower_ident_opt,
};
use crate::{
    container::{ContainerId, InFile},
    db::InternDb,
    define_src_with_name,
    file::HirFileId,
    hir_def::{alloc_idx_and_src, lower_named_label_opt},
    source_map::SourceMap,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Stmt {
    pub label: Option<Ident>,
    pub kind: StmtKind,
}

pub type StmtId = Idx<Stmt>;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub enum StmtKind {
    #[default]
    Empty,

    Expr(ExprId),
    TimingCtrl(TimingControl, StmtId),
    ProcAssign(ProcAssignKind),
    EventTrigger(EventTrigger),
    Block(BlockInfo),

    Cond {
        unique_priority: Option<UniquePriority>,
        pred: SmallVec<[ExprId; 1]>,
        then_stmt: StmtId,
        else_stmt: Option<StmtId>,
    },
    Case {
        unique_priority: Option<UniquePriority>,
        case: Option<CaseKeyword>,
        expr: ExprId,
        items: SmallVec<[CaseItem; 5]>,
    },

    Forever(StmtId),
    DoWhile(StmtId, ExprId),
    Repeat(ExprId, StmtId),
    While(ExprId, StmtId),
    For {
        inits: ForInit,
        stop: ExprId,
        steps: SmallVec<[ExprId; 1]>,
        stmt: StmtId,
    },
    Jump(JumpKind),

    Wait(WaitKind, StmtId),
    Disable(DisableKind),
    Opaque,
}

define_src_with_name!(StmtSrc(ast::Statement));

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ProcAssignKind {
    Assign(ExprId),
    Force(ExprId),
    Deassign(ExprId),
    Release(ExprId),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct EventTrigger {
    pub kind: EventTriggerKind,
    pub timing: Option<TimingControl>,
    pub event: ExprId,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum EventTriggerKind {
    Blocking,
    Nonblocking,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum JumpKind {
    Return(Option<ExprId>),
    Break,
    Continue,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ForInit {
    Init(SmallVec<[(DataTy, DeclId); 1]>),
    Assign(SmallVec<[ExprId; 1]>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum WaitKind {
    Wait(ExprId),
    // TODO: more wait statements
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum DisableKind {
    Disable(ExprId),
    // TODO: more disable statements
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum UniquePriority {
    Unique,
    Unique0,
    Priority,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum CaseKeyword {
    Case,
    Casez,
    Casex,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum CaseItem {
    Case { exprs: SmallVec<[ExprId; 1]>, clause: StmtId },
    Default(StmtId),
}

pub(crate) trait LowerStmt: LowerExpr + LowerEventExpr + LowerDecl {
    fn stmt_ctx(&mut self) -> LowerStmtCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_stmt {
    ($ctx:ty, $cont_id:ident $(,$data:ident, $src_map:ident)?) => {
        impl $crate::hir_def::stmt::LowerStmt for $ctx {
            fn stmt_ctx(&mut self) -> $crate::hir_def::stmt::LowerStmtCtx<'_> {
                $crate::hir_def::stmt::LowerStmtCtx {
                    db: self.db,
                    file_id: self.file_id,
                    cont_id: self.$cont_id.into(),
                    stmts: &mut self.$($data.)?stmts,
                    stmt_srcs: &mut self.$($src_map.)?stmt_srcs,
                    event_exprs: &mut self.$($data.)?event_exprs,
                    event_expr_srcs: &mut self.$($src_map.)?event_expr_srcs,
                    exprs: &mut self.$($data.)?exprs,
                    expr_srcs: &mut self.$($src_map.)?expr_srcs,
                    decls: &mut self.$($data.)?decls,
                    decl_srcs: &mut self.$($src_map.)?decl_srcs,
                }
            }
        }
    }
}

pub(crate) struct LowerStmtCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) cont_id: ContainerId,

    pub(crate) stmts: &'a mut Arena<Stmt>,
    pub(crate) stmt_srcs: &'a mut SourceMap<StmtSrc, Stmt>,

    pub(crate) event_exprs: &'a mut Arena<EventExpr>,
    pub(crate) event_expr_srcs: &'a mut SourceMap<EventExprSrc, EventExpr>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,

    pub(crate) decls: &'a mut Arena<Declarator>,
    pub(crate) decl_srcs: &'a mut SourceMap<DeclaratorSrc, Declarator>,
}

impl_lower_expr!(LowerStmtCtx<'_>);
impl_lower_decl!(LowerStmtCtx<'_>);
impl_lower_event_expr!(LowerStmtCtx<'_>);

impl LowerStmtCtx<'_> {
    pub(crate) fn lower_stmt_opt(&mut self, stmt: Option<ast::Statement>) -> StmtId {
        if let Some(stmt) = stmt { self.lower_stmt(stmt) } else { self.alloc_missing() }
    }

    pub(crate) fn lower_stmt(&mut self, stmt: ast::Statement) -> StmtId {
        let hir_stmt = self.lower_stmt_inner(stmt);
        alloc_idx_and_src! {
            hir_stmt => self.stmts,
            stmt => self.stmt_srcs,
        }
    }

    fn lower_stmt_inner(&mut self, stmt: ast::Statement) -> Stmt {
        let label = lower_named_label_opt(stmt.label());

        use ast::Statement::*;
        let kind = match stmt {
            ExpressionStatement(stmt) => self.lower_expr_stmt(stmt),
            TimingControlStatement(stmt) => self.lower_timing_ctrl_stmt(stmt),
            ProceduralAssignStatement(stmt) => self.lower_assign_stmt(stmt),
            ProceduralDeassignStatement(stmt) => self.lower_deassign_stmt(stmt),
            EventTriggerStatement(stmt) => self.lower_event_trigger_stmt(stmt),

            WaitStatement(stmt) => self.lower_wait_stmt(stmt),
            DisableStatement(stmt) => self.lower_disable_stmt(stmt),

            ConditionalStatement(stmt) => self.lower_cond_stmt(stmt),
            CaseStatement(stmt) => self.lower_case_stmt(stmt),

            ReturnStatement(stmt) => self.lower_return_stmt(stmt),
            DoWhileStatement(stmt) => self.lower_do_while_stmt(stmt),
            ForeverStatement(stmt) => self.lower_forever_stmt(stmt),
            LoopStatement(stmt) => self.lower_loop_stmt(stmt),
            JumpStatement(stmt) => self.lower_jump_stmt(stmt),
            ForLoopStatement(stmt) => self.lower_for_loop_stmt(stmt),

            BlockStatement(stmt) => self.lower_block_stmt(stmt),

            EmptyStatement(_) => StmtKind::Empty,

            // SystemVerilog-only statement forms remain model-limited. Keep these explicit so
            // new or accidentally uncovered statement variants cannot silently degrade to opaque.
            ConcurrentAssertionStatement(_)
            | CheckerInstanceStatement(_)
            | DisableForkStatement(_)
            | ForeachLoopStatement(_)
            | ImmediateAssertionStatement(_)
            | RandCaseStatement(_)
            | RandSequenceStatement(_)
            | VoidCastedCallStatement(_)
            | WaitForkStatement(_)
            | WaitOrderStatement(_) => StmtKind::Opaque,
        };

        Stmt { label, kind }
    }

    fn lower_expr_stmt(&mut self, stmt: ast::ExpressionStatement) -> StmtKind {
        let expr = self.expr_ctx().lower_expr(stmt.expr());
        StmtKind::Expr(expr)
    }

    fn lower_assign_stmt(&mut self, stmt: ast::ProceduralAssignStatement) -> StmtKind {
        let expr = self.expr_ctx().lower_expr(stmt.expr());

        use ast::ProceduralAssignStatement::*;
        let kind = match stmt {
            ProceduralForceStatement(_) => ProcAssignKind::Force(expr),
            ProceduralAssignStatement(_) => ProcAssignKind::Assign(expr),
        };

        StmtKind::ProcAssign(kind)
    }

    fn lower_deassign_stmt(&mut self, stmt: ast::ProceduralDeassignStatement) -> StmtKind {
        let expr = self.expr_ctx().lower_expr(stmt.variable());

        use ast::ProceduralDeassignStatement::*;
        let kind = match stmt {
            ProceduralDeassignStatement(_) => ProcAssignKind::Deassign(expr),
            ProceduralReleaseStatement(_) => ProcAssignKind::Release(expr),
        };

        StmtKind::ProcAssign(kind)
    }

    fn lower_event_trigger_stmt(&mut self, stmt: ast::EventTriggerStatement) -> StmtKind {
        let event = ast::Expression::cast(stmt.name().syntax())
            .map(|expr| self.expr_ctx().lower_expr(expr))
            .unwrap_or_else(|| self.expr_ctx().lower_expr_opt(None));
        let timing = stmt.timing().map(|timing| self.event_expr_ctx().lower_timing_control(timing));

        let kind = match stmt {
            ast::EventTriggerStatement::BlockingEventTriggerStatement(_) => {
                EventTriggerKind::Blocking
            }
            ast::EventTriggerStatement::NonblockingEventTriggerStatement(_) => {
                EventTriggerKind::Nonblocking
            }
        };

        StmtKind::EventTrigger(EventTrigger { kind, timing, event })
    }

    fn lower_forever_stmt(&mut self, stmt: ast::ForeverStatement) -> StmtKind {
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::Forever(stmt)
    }

    fn lower_do_while_stmt(&mut self, stmt: ast::DoWhileStatement) -> StmtKind {
        let expr = self.expr_ctx().lower_expr(stmt.expr());
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::DoWhile(stmt, expr)
    }

    fn lower_for_loop_stmt(&mut self, stmt: ast::ForLoopStatement) -> StmtKind {
        let mut initializers = stmt.initializers().children().peekable();

        let inits = match initializers.peek().map(|init| init.syntax().kind()) {
            Some(SyntaxKind::FOR_VARIABLE_DECLARATION) => {
                let mut ty = None;
                let mut inits = SmallVec::new();
                let next_stmt_id = self.stmts.nxt_idx().into();
                for init in initializers {
                    let init = ast::ForVariableDeclaration::cast(init.syntax()).unwrap();
                    if let Some(ast_ty) = init.type_() {
                        ty = Some(self.expr_ctx().lower_data_ty(ast_ty));
                    }
                    let decl = self.decl_ctx().lower_declarator(init.declarator(), next_stmt_id);
                    inits.push((ty.unwrap(), decl));
                }
                ForInit::Init(inits)
            }
            Some(SyntaxKind::ASSIGNMENT_EXPRESSION) => {
                let inits = initializers
                    .map(|init| {
                        let expr = ast::Expression::cast(init.syntax()).unwrap();
                        self.expr_ctx().lower_expr(expr)
                    })
                    .collect();
                ForInit::Assign(inits)
            }
            None => ForInit::Assign(SmallVec::new()),
            _ => unreachable!(),
        };

        let stop = self.expr_ctx().lower_expr_opt(stmt.stop_expr());
        let steps = stmt.steps().children().map(|step| self.expr_ctx().lower_expr(step)).collect();
        let stmt = self.lower_stmt(stmt.statement());

        StmtKind::For { inits, stop, steps, stmt }
    }

    fn lower_return_stmt(&mut self, stmt: ast::ReturnStatement) -> StmtKind {
        let expr = stmt.return_value().map(|expr| self.expr_ctx().lower_expr(expr));
        StmtKind::Jump(JumpKind::Return(expr))
    }

    fn lower_loop_stmt(&mut self, stmt: ast::LoopStatement) -> StmtKind {
        let expr = self.expr_ctx().lower_expr(stmt.expr());
        let body = self.lower_stmt(stmt.statement());
        match stmt.repeat_or_while().map(|tok| tok.kind()) {
            Some(TokenKind::REPEAT_KEYWORD) => StmtKind::Repeat(expr, body),
            Some(TokenKind::WHILE_KEYWORD) | None => StmtKind::While(expr, body),
            _ => unreachable!(),
        }
    }

    fn lower_wait_stmt(&mut self, stmt: ast::WaitStatement) -> StmtKind {
        let expr = self.expr_ctx().lower_expr(stmt.expr());
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::Wait(WaitKind::Wait(expr), stmt)
    }

    fn lower_disable_stmt(&mut self, stmt: ast::DisableStatement) -> StmtKind {
        let name = ast::Expression::cast(stmt.name().syntax()).unwrap();
        let name = self.expr_ctx().lower_expr(name);
        StmtKind::Disable(DisableKind::Disable(name))
    }

    fn lower_jump_stmt(&mut self, stmt: ast::JumpStatement) -> StmtKind {
        let kind = match stmt.break_or_continue().unwrap().kind() {
            TokenKind::BREAK_KEYWORD => JumpKind::Break,
            TokenKind::CONTINUE_KEYWORD => JumpKind::Continue,
            _ => unreachable!(),
        };
        StmtKind::Jump(kind)
    }

    fn lower_cond_stmt(&mut self, stmt: ast::ConditionalStatement) -> StmtKind {
        let unique_priority = lower_unique_or_priority(stmt.unique_or_priority());
        let pred = stmt
            .predicate()
            .conditions()
            .children()
            .map(|cond| self.expr_ctx().lower_expr(cond.expr()))
            .collect();
        let then_stmt = self.lower_stmt(stmt.statement());
        let else_stmt = stmt
            .else_clause()
            .and_then(|clause| ast::Statement::cast(clause.clause().syntax()))
            .map(|stmt| self.lower_stmt(stmt));
        StmtKind::Cond { unique_priority, pred, then_stmt, else_stmt }
    }

    fn lower_timing_ctrl_stmt(&mut self, stmt: ast::TimingControlStatement) -> StmtKind {
        let timing_control = self.event_expr_ctx().lower_timing_control(stmt.timing_control());
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::TimingCtrl(timing_control, stmt)
    }

    fn lower_case_stmt(&mut self, stmt: ast::CaseStatement) -> StmtKind {
        let unique_priority = lower_unique_or_priority(stmt.unique_or_priority());

        let case = stmt.case_keyword().map(|case| match case.kind() {
            TokenKind::CASE_KEYWORD => CaseKeyword::Case,
            TokenKind::CASE_Z_KEYWORD => CaseKeyword::Casez,
            TokenKind::CASE_X_KEYWORD => CaseKeyword::Casex,
            _ => unreachable!(),
        });

        let expr = self.expr_ctx().lower_expr(stmt.expr());

        let items = stmt
            .items()
            .children()
            .map(|item| {
                use ast::CaseItem::*;
                match item {
                    DefaultCaseItem(item) => {
                        let clause = ast::Statement::cast(item.clause().syntax());
                        let default = self.lower_stmt_opt(clause);
                        CaseItem::Default(default)
                    }
                    StandardCaseItem(item) => {
                        let exprs = item
                            .expressions()
                            .children()
                            .map(|expr| self.expr_ctx().lower_expr(expr))
                            .collect();
                        let clause =
                            self.lower_stmt_opt(ast::Statement::cast(item.clause().syntax()));
                        CaseItem::Case { exprs, clause }
                    }
                    PatternCaseItem(item) => {
                        let mut exprs = SmallVec::new();
                        if let Some(expr) = item.expr() {
                            exprs.push(self.expr_ctx().lower_expr(expr));
                        }
                        let clause = self.lower_stmt(item.statement());
                        CaseItem::Case { exprs, clause }
                    }
                }
            })
            .collect();

        StmtKind::Case { unique_priority, case, expr, items }
    }

    fn lower_block_stmt(&mut self, stmt: ast::BlockStatement) -> StmtKind {
        let loc = BlockLoc { cont_id: self.cont_id, src: InFile::new(self.file_id, stmt.into()) };
        let block_id = self.db.intern_block(loc);
        let name = stmt.block_name().and_then(|name| lower_ident_opt(name.name()));
        StmtKind::Block(BlockInfo { name, block_id })
    }

    fn alloc_missing(&mut self) -> StmtId {
        self.stmts.alloc(Stmt { label: None, kind: StmtKind::Empty })
    }
}

fn lower_unique_or_priority(up: Option<SyntaxToken>) -> Option<UniquePriority> {
    match up?.kind() {
        TokenKind::UNIQUE_KEYWORD => Some(UniquePriority::Unique),
        TokenKind::UNIQUE_0_KEYWORD => Some(UniquePriority::Unique0),
        TokenKind::PRIORITY_KEYWORD => Some(UniquePriority::Priority),
        TokenKind::UNKNOWN => None,
        _ => unreachable!(),
    }
}
