use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, SyntaxToken, TokenKind,
    ast::{self, AstNode},
};

use super::{
    Ident,
    block::{BlockInfo, BlockLoc, BlockSrc},
    expr::{ExprId, data_ty::DataTy, declarator::DeclId, timing_control::TimingControl},
    lower::{LoweringCtx, LoweringStore},
    lower_ident_opt,
};
use crate::{
    container::InFile,
    hir_def::{alloc_with_source, lower_named_label_opt},
    source_map::{AstKind, NamedAstId},
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
    Missing,
    Invalid,
    Unsupported(SyntaxKind),
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
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct StatementAst;

impl AstKind for StatementAst {
    type Node<'a> = ast::Statement<'a>;
}

pub type StmtSrc = NamedAstId<StatementAst>;

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
    Init(SmallVec<[(Option<DataTy>, DeclId); 1]>),
    Assign(SmallVec<[ExprId; 1]>),
    Missing,
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

impl<Store: LoweringStore> LoweringCtx<'_, Store> {
    pub(crate) fn lower_stmt_opt(&mut self, stmt: Option<ast::Statement>) -> StmtId {
        if let Some(stmt) = stmt { self.lower_stmt(stmt) } else { self.alloc_missing() }
    }

    pub(crate) fn lower_stmt(&mut self, stmt: ast::Statement) -> StmtId {
        let label = lower_named_label_opt(stmt.label());
        let file_id = self.file_id;
        let (statements, sources) = self.statements();
        let stmt_id = alloc_with_source(
            file_id,
            statements,
            sources,
            Stmt { label, kind: StmtKind::Empty },
            stmt,
        );
        let kind = self.lower_stmt_kind(stmt, stmt_id);
        self.statements().0[stmt_id].kind = kind;
        stmt_id
    }

    fn lower_stmt_kind(&mut self, stmt: ast::Statement, stmt_id: StmtId) -> StmtKind {
        use ast::Statement::*;
        match stmt {
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
            ForLoopStatement(stmt) => self.lower_for_loop_stmt(stmt, stmt_id),

            BlockStatement(stmt) => self.lower_block_stmt(stmt),

            EmptyStatement(_) => StmtKind::Empty,

            unsupported => StmtKind::Unsupported(unsupported.syntax().kind()),
        }
    }

    fn lower_expr_stmt(&mut self, stmt: ast::ExpressionStatement) -> StmtKind {
        let expr = self.lower_expr(stmt.expr());
        StmtKind::Expr(expr)
    }

    fn lower_assign_stmt(&mut self, stmt: ast::ProceduralAssignStatement) -> StmtKind {
        let expr = self.lower_expr(stmt.expr());

        use ast::ProceduralAssignStatement::*;
        let kind = match stmt {
            ProceduralForceStatement(_) => ProcAssignKind::Force(expr),
            ProceduralAssignStatement(_) => ProcAssignKind::Assign(expr),
        };

        StmtKind::ProcAssign(kind)
    }

    fn lower_deassign_stmt(&mut self, stmt: ast::ProceduralDeassignStatement) -> StmtKind {
        let expr = self.lower_expr(stmt.variable());

        use ast::ProceduralDeassignStatement::*;
        let kind = match stmt {
            ProceduralDeassignStatement(_) => ProcAssignKind::Deassign(expr),
            ProceduralReleaseStatement(_) => ProcAssignKind::Release(expr),
        };

        StmtKind::ProcAssign(kind)
    }

    fn lower_event_trigger_stmt(&mut self, stmt: ast::EventTriggerStatement) -> StmtKind {
        let event = ast::Expression::cast(stmt.name().syntax())
            .map(|expr| self.lower_expr(expr))
            .unwrap_or_else(|| self.lower_expr_opt(None));
        let timing = stmt.timing().map(|timing| self.lower_timing_control(timing));

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
        let expr = self.lower_expr(stmt.expr());
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::DoWhile(stmt, expr)
    }

    fn lower_for_loop_stmt(&mut self, stmt: ast::ForLoopStatement, stmt_id: StmtId) -> StmtKind {
        let mut initializers = stmt.initializers().children().peekable();

        let inits = match initializers.peek().map(|init| init.syntax().kind()) {
            Some(SyntaxKind::FOR_VARIABLE_DECLARATION) => {
                let mut ty = None;
                let mut inits = SmallVec::new();
                let parent = stmt_id.into();
                for init in initializers {
                    let Some(init) = ast::ForVariableDeclaration::cast(init.syntax()) else {
                        continue;
                    };
                    if let Some(ast_ty) = init.type_() {
                        ty = Some(self.lower_data_ty(ast_ty));
                    }
                    let decl = self.lower_declarator(init.declarator(), parent);
                    inits.push((ty, decl));
                }
                ForInit::Init(inits)
            }
            Some(SyntaxKind::ASSIGNMENT_EXPRESSION) => {
                let inits = initializers
                    .filter_map(|init| {
                        ast::Expression::cast(init.syntax()).map(|expr| self.lower_expr(expr))
                    })
                    .collect();
                ForInit::Assign(inits)
            }
            None => ForInit::Assign(SmallVec::new()),
            _ => ForInit::Missing,
        };

        let stop = self.lower_expr_opt(stmt.stop_expr());
        let steps = stmt.steps().children().map(|step| self.lower_expr(step)).collect();
        let stmt = self.lower_stmt(stmt.statement());

        StmtKind::For { inits, stop, steps, stmt }
    }

    fn lower_return_stmt(&mut self, stmt: ast::ReturnStatement) -> StmtKind {
        let expr = stmt.return_value().map(|expr| self.lower_expr(expr));
        StmtKind::Jump(JumpKind::Return(expr))
    }

    fn lower_loop_stmt(&mut self, stmt: ast::LoopStatement) -> StmtKind {
        let expr = self.lower_expr(stmt.expr());
        let body = self.lower_stmt(stmt.statement());
        match stmt.repeat_or_while().map(|tok| tok.kind()) {
            Some(TokenKind::REPEAT_KEYWORD) => StmtKind::Repeat(expr, body),
            Some(TokenKind::WHILE_KEYWORD) | None => StmtKind::While(expr, body),
            _ => StmtKind::Invalid,
        }
    }

    fn lower_wait_stmt(&mut self, stmt: ast::WaitStatement) -> StmtKind {
        let expr = self.lower_expr(stmt.expr());
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::Wait(WaitKind::Wait(expr), stmt)
    }

    fn lower_disable_stmt(&mut self, stmt: ast::DisableStatement) -> StmtKind {
        let name = ast::Expression::cast(stmt.name().syntax())
            .map(|name| self.lower_expr(name))
            .unwrap_or_else(|| self.lower_expr_opt(None));
        StmtKind::Disable(DisableKind::Disable(name))
    }

    fn lower_jump_stmt(&mut self, stmt: ast::JumpStatement) -> StmtKind {
        let Some(kind) = stmt.break_or_continue().and_then(|tok| match tok.kind() {
            TokenKind::BREAK_KEYWORD => Some(JumpKind::Break),
            TokenKind::CONTINUE_KEYWORD => Some(JumpKind::Continue),
            _ => None,
        }) else {
            return StmtKind::Invalid;
        };
        StmtKind::Jump(kind)
    }

    fn lower_cond_stmt(&mut self, stmt: ast::ConditionalStatement) -> StmtKind {
        let unique_priority = lower_unique_or_priority(stmt.unique_or_priority());
        let pred = stmt
            .predicate()
            .conditions()
            .children()
            .map(|cond| self.lower_expr(cond.expr()))
            .collect();
        let then_stmt = self.lower_stmt(stmt.statement());
        let else_stmt = stmt
            .else_clause()
            .and_then(|clause| ast::Statement::cast(clause.clause().syntax()))
            .map(|stmt| self.lower_stmt(stmt));
        StmtKind::Cond { unique_priority, pred, then_stmt, else_stmt }
    }

    fn lower_timing_ctrl_stmt(&mut self, stmt: ast::TimingControlStatement) -> StmtKind {
        let timing_control = self.lower_timing_control(stmt.timing_control());
        let stmt = self.lower_stmt(stmt.statement());
        StmtKind::TimingCtrl(timing_control, stmt)
    }

    fn lower_case_stmt(&mut self, stmt: ast::CaseStatement) -> StmtKind {
        let unique_priority = lower_unique_or_priority(stmt.unique_or_priority());

        let case = stmt.case_keyword().and_then(|case| match case.kind() {
            TokenKind::CASE_KEYWORD => Some(CaseKeyword::Case),
            TokenKind::CASE_Z_KEYWORD => Some(CaseKeyword::Casez),
            TokenKind::CASE_X_KEYWORD => Some(CaseKeyword::Casex),
            _ => None,
        });

        let expr = self.lower_expr(stmt.expr());

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
                            .map(|expr| self.lower_expr(expr))
                            .collect();
                        let clause =
                            self.lower_stmt_opt(ast::Statement::cast(item.clause().syntax()));
                        CaseItem::Case { exprs, clause }
                    }
                    PatternCaseItem(item) => {
                        let mut exprs = SmallVec::new();
                        if let Some(expr) = item.expr() {
                            exprs.push(self.lower_expr(expr));
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
        let loc = BlockLoc {
            cont_id: self.owner,
            src: InFile::new(self.file_id, BlockSrc::from_ast(self.file_id, stmt)),
        };
        let block_id = self.db.intern_block(loc);
        let name = stmt.block_name().and_then(|name| lower_ident_opt(name.name()));
        StmtKind::Block(BlockInfo { name, block_id })
    }

    fn alloc_missing(&mut self) -> StmtId {
        self.statements().0.alloc(Stmt { label: None, kind: StmtKind::Missing })
    }
}

fn lower_unique_or_priority(up: Option<SyntaxToken>) -> Option<UniquePriority> {
    match up?.kind() {
        TokenKind::UNIQUE_KEYWORD => Some(UniquePriority::Unique),
        TokenKind::UNIQUE_0_KEYWORD => Some(UniquePriority::Unique0),
        TokenKind::PRIORITY_KEYWORD => Some(UniquePriority::Priority),
        TokenKind::UNKNOWN => None,
        _ => None,
    }
}
