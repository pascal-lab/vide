use crate::{
    container::InFile,
    hir_def::{
        block::BlockInfo,
        control::{DelayOrEventControl, LowerTimingControl, ProceduralTimingControlControl},
        expr::{self, AssignOp, ExprId, LowerExpr},
        try_match, Ident, SourceMap,
    },
};
use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::ast::{self, ptr};

use super::{
    block::{
        block_src::{BlockSrc, LocalBlockSrc},
        BlockItemDecl, BlockLoc,
    },
    data::LowerDataDecl,
};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Assign {
    pub lhs: ExprId,
    pub rhs: ExprId,
    pub op: AssignOp,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ProceduralContinuousAssign {
    Assign(Assign),
    Deassign(ExprId),
    Force(Assign),
    Release(ExprId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    pub ident: Option<Ident>,
    pub item: StmtItem,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum UniquePriority {
    Unique,
    Unique0,
    Priority,
}

pub(crate) fn lower_unique_priority(priority: &ast::UniquePriority) -> Option<UniquePriority> {
    try_match! {
        priority.token_unique(), _ => Some(UniquePriority::Unique),
        priority.token_unique0(), _ => Some(UniquePriority::Unique0),
        priority.token_priority(), _ => Some(UniquePriority::Priority),
        _ => None,
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CondPredicate(SmallVec<[ExprOrCondPat; 1]>);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ExprOrCondPat {
    Expr(ExprId),
    // TODO: CondPat(CondPredicate),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CaseKetword {
    Case,
    Casez,
    Casex,
}

pub(crate) fn lower_case_keyword(keyword: &ast::CaseKeyword) -> Option<CaseKetword> {
    try_match! {
        keyword.token_case(), _ => Some(CaseKetword::Case),
        keyword.token_casez(), _ => Some(CaseKetword::Casez),
        keyword.token_casex(), _ => Some(CaseKetword::Casex),
        _ => None,
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CaseItem {
    Case { exprs: SmallVec<[ExprId; 1]>, stmt: Option<StmtId> },
    Default(Option<StmtId>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CaseItems {
    Case(SmallVec<[CaseItem; 1]>),
    // TODO: Pattern()
    // TODO: Inside()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StmtItem {
    BlockingAssign {
        control: Option<DelayOrEventControl>,
        assign: Assign,
    },
    NonblockingAssign {
        control: Option<DelayOrEventControl>,
        assign: Assign,
    },
    ProceduralContinuousAssign(ProceduralContinuousAssign),
    CaseStmt {
        unique_priority: Option<UniquePriority>,
        case_keyword: CaseKetword,
        case_expr: ExprId,
        case_items: CaseItems,
    },
    CondStmt {
        unique_priority: Option<UniquePriority>,
        cond_predict: CondPredicate,
        stmt: Option<StmtId>,
        else_stmt: Option<StmtId>,
    },
    BlockInfo(Idx<BlockInfo>),
    ProceduralTimingControlStmt {
        control: ProceduralTimingControlControl,
        stmt: Option<StmtId>,
    },
}

pub type StmtId = Idx<Stmt>;
pub type StmtSrc = InFile<ptr::StatementPtr>;

pub(crate) trait LowerStmt: LowerTimingControl + LowerExpr + LowerDataDecl {
    fn arena_stmts(&mut self) -> &mut Arena<Stmt>;

    fn arena_blocks(&mut self) -> &mut Arena<BlockInfo>;

    fn src_map_stmt(&mut self) -> &mut SourceMap<StmtSrc, Stmt>;

    fn src_map_blocks(&mut self) -> &mut SourceMap<BlockSrc, BlockInfo>;

    fn lower_stmt(&mut self, stmt: &ast::Statement) -> Option<StmtId> {
        let ident = stmt.identifier().and_then(|ident| self.lower_ident(&ident));
        let item = self.lower_stmt_item(&stmt.statement_item()?)?;
        let src = self.in_file(stmt.to_ptr());
        let idx = self.arena_stmts().alloc(Stmt { ident, item });
        self.src_map_stmt().insert(src, idx);
        Some(idx)
    }

    fn lower_stmt_or_null(&mut self, stmt_or_null: &ast::StatementOrNull) -> Option<StmtId> {
        try_match! {
            stmt_or_null.statement(), stmt => {
                self.lower_stmt(&stmt)
            },
            _ => None,
        }
    }

    fn lower_stmt_item(&mut self, stmt: &ast::StatementItem) -> Option<StmtItem> {
        try_match! {
            stmt.blocking_assignment(), assign => {
                let control = try_match!{
                    assign.delay_or_event_control(), control => {
                        self.lower_delay_or_event_control(&control)
                    },
                    _ => None
                };
                let assign = try_match!{
                    assign.variable_lvalue(), var_lvalue => {
                        let lhs = self.lower_var_lvalue(&var_lvalue)?;
                        let rhs = self.lower_expr(&assign.expression()?);
                        let op = AssignOp::Assign;
                        Assign { lhs, rhs, op }
                    },
                    assign.nonrange_variable_lvalue(), _ => {
                        unimplemented!("nonrange_variable_lvalue = dynamic_array_new")
                    },
                    // TODO: add syntax: hierarchical_variable_identifier select = class_new
                    assign.operator_assignment(), op_assign => {
                        self.lower_op_assign(&op_assign)?
                    },
                    _ => { return None; }
                };
                Some(StmtItem::BlockingAssign{ control, assign })
            },
            stmt.nonblocking_assignment(), assign => {
                let control = assign.delay_or_event_control().and_then(|control| self.lower_delay_or_event_control(&control));
                let assign = Assign {
                    lhs: self.lower_var_lvalue(&assign.variable_lvalue()?)?,
                    rhs: self.lower_expr(&assign.expression()?),
                    op: AssignOp::Assign,
                };
                Some(StmtItem::NonblockingAssign{ control, assign })
            },
            stmt.procedural_continuous_assignment(), assign => {
                let assign = try_match! {
                    assign.token_assign(), _ => {
                        ProceduralContinuousAssign::Assign(
                            self.lower_var_assign(&assign.variable_assignment()?)?
                        )
                    },
                    assign.token_deassign(), _ => {
                        ProceduralContinuousAssign::Deassign(
                            self.lower_var_lvalue(&assign.variable_lvalue()?)?
                        )
                    },
                    assign.token_force(), _ => {
                        let assign = try_match!{
                            assign.variable_assignment(), var_assign => {
                                self.lower_var_assign(&var_assign)?
                            },
                            assign.net_assignment(), net_assign => {
                                self.lower_net_assign(&net_assign)?
                            },
                            _ => { return None; }
                        };
                        ProceduralContinuousAssign::Force(assign)
                    },
                    assign.token_release(), _ => {
                        let lvalue = try_match!{
                            assign.variable_lvalue(), var_lvalue => {
                                self.lower_var_lvalue(&var_lvalue)?
                            },
                            assign.net_lvalue(), net_lvalue => {
                                self.lower_net_lvalue(&net_lvalue)?
                            },
                            _ => { return None; }
                        };
                        ProceduralContinuousAssign::Release(lvalue)
                    },
                    _ => { return None; }
                };
                Some(StmtItem::ProceduralContinuousAssign(assign))
            },
            stmt.case_statement(), case => {
                let unique_priority = case.unique_priority().and_then(|priority| lower_unique_priority(&priority));
                let case_keyword = lower_case_keyword(&case.case_keyword()?)?;
                let case_expr = self.lower_expr(&case.case_expression()?.expression()?);
                let case_items = try_match! {
                    case.token_matches(), _ => {
                        unimplemented!("case_statement with matches")
                    },
                    case.token_inside(), _ => {
                        unimplemented!("case_statement with inside")
                    },
                    _ => {
                        let mut items: SmallVec<[CaseItem; 1]> = SmallVec::new();
                        for case_item in case.case_items() {
                            let item = try_match! {
                                case_item.token_default(), _ => {
                                    CaseItem::Default(self.lower_stmt_or_null(&case_item.statement_or_null()?))
                                },
                                _ => {
                                    let mut exprs: SmallVec<[ExprId; 1]> = SmallVec::new();
                                    for expr in case_item.case_item_expressions() {
                                        exprs.push(self.lower_expr(&expr.expression()?));
                                    }
                                    let stmt = self.lower_stmt_or_null(&case_item.statement_or_null()?);
                                    CaseItem::Case{ exprs, stmt }
                                }
                            };
                            items.push(item);
                        }
                        CaseItems::Case(items)
                    }
                };
                Some(StmtItem::CaseStmt {
                    unique_priority,
                    case_keyword,
                    case_expr,
                    case_items
                })
            },
            stmt.conditional_statement(), cond => {
                let unique_priority = cond.unique_priority().and_then(|priority| lower_unique_priority(&priority));
                let mut cond_predict: SmallVec<[ExprOrCondPat; 1]> = SmallVec::new();
                let cond_predicte = cond.cond_predicate()?;
                for cond_or_cond_pat in cond_predicte.expression_or_cond_patterns() {
                    let expr_or_cond_pat = try_match! {
                        cond_or_cond_pat.expression(), expr => {
                            ExprOrCondPat::Expr(self.lower_expr(&expr))
                        },
                        _ => { return None; }
                    };
                    cond_predict.push(expr_or_cond_pat);
                }
                let cond_predict = CondPredicate(cond_predict);
                let mut iter = cond.statement_or_nulls();
                let stmt = self.lower_stmt_or_null(&iter.next()?);
                let else_stmt = self.lower_stmt_or_null(&iter.next()?);
                Some(StmtItem::CondStmt { unique_priority, cond_predict, stmt, else_stmt })
            },
            stmt.seq_block(), seq_block => {
                Some(StmtItem::BlockInfo(self.lower_seq_block(&seq_block)?))
            },
            stmt.par_block(), par_block => {
                Some(StmtItem::BlockInfo(self.lower_par_block(&par_block)?))
            },
            stmt.inc_or_dec_expression(), _inc_or_dec => {
                unimplemented!("inc_or_dec_expression")
            },
            // TODO: add syntax: subroutine_call_statement
            stmt.disable_statement(), _disable => {
                unimplemented!("disable_statement")
            },
            stmt.event_trigger(), _event_trigger => {
                unimplemented!("event_trigger")
            },
            stmt.loop_statement(), _loop => {
                unimplemented!("loop_statement")
            },
            stmt.jump_statement(), _jump => {
                unimplemented!("jump_statement")
            },
            stmt.procedural_timing_control_statement(), control => {
                Some(StmtItem::ProceduralTimingControlStmt {
                    control: self.lower_procedural_timing_control(&control.procedural_timing_control()?)?,
                    stmt: self.lower_stmt_or_null(&control.statement_or_null()?),
                })
            },
            stmt.wait_statement(), _wait => {
                unimplemented!("wait_statement")
            },
            stmt.procedural_assertion_statement(), _assertion => {
                unimplemented!("procedural_assertion_statement")
            },
            stmt.clocking_drive(), _clocking => {
                unimplemented!("clocking_drive")
            },
            // TODO: Add syntax randsequence_statement
            stmt.randcase_statement(), _randcase => {
                unimplemented!("randcase_statement")
            },
            stmt.expect_property_statement(), _expect_property => {
                unimplemented!("expect_property_statement")
            },
            _ => None,
        }
    }

    fn lower_net_assign(&mut self, net_assign: &ast::NetAssignment) -> Option<Assign> {
        let lhs = self.lower_net_lvalue(&net_assign.net_lvalue()?)?;
        let rhs = self.lower_expr(&net_assign.expression()?);
        let op = AssignOp::Assign;
        Some(Assign { lhs, rhs, op })
    }

    fn lower_var_assign(&mut self, var_assign: &ast::VariableAssignment) -> Option<Assign> {
        let lhs = self.lower_var_lvalue(&var_assign.variable_lvalue()?)?;
        let rhs = self.lower_expr(&var_assign.expression()?);
        let op = AssignOp::Assign;
        Some(Assign { lhs, rhs, op })
    }

    fn lower_op_assign(&mut self, op_assign: &ast::OperatorAssignment) -> Option<Assign> {
        let lhs = self.lower_var_lvalue(&op_assign.variable_lvalue()?)?;
        let rhs = self.lower_expr(&op_assign.expression()?);
        let op = try_match! {
            op_assign.assignment_operator(), op => {
                expr::lower_assign_op(&op)?
            },
            _ => { return None; }
        };
        Some(Assign { lhs, rhs, op })
    }

    fn lower_block_item_decl(&mut self, item: &ast::BlockItemDeclaration) -> Option<BlockItemDecl> {
        try_match! {
            item.data_declaration(), data_decl => {
                Some(BlockItemDecl::DataDecl(self.lower_data_decl(&data_decl)?))
            },
            item.any_parameter_declaration(), any_param_decl => {
                Some(BlockItemDecl::DataDecl(self.lower_any_param_decl(&any_param_decl)?))
            },
            _ => None,
        }
    }

    fn lower_seq_block(&mut self, block: &ast::SeqBlock) -> Option<Idx<BlockInfo>> {
        let ident = block.identifiers().next().and_then(|ident| self.lower_ident(&ident));
        let block_id = self.db().intern_block(BlockLoc {
            container_id: self.container_id().into(),
            block_src: self.in_file(LocalBlockSrc::SeqBlock(block.to_ptr())),
        });
        let idx = self.arena_blocks().alloc(BlockInfo { ident, block_id });
        let src = self.in_file(LocalBlockSrc::SeqBlock(block.to_ptr()));
        self.src_map_blocks().insert(src, idx);
        Some(idx)
    }

    fn lower_par_block(&mut self, block: &ast::ParBlock) -> Option<Idx<BlockInfo>> {
        let ident = block.identifiers().next().and_then(|ident| self.lower_ident(&ident));
        let block_id = self.db().intern_block(BlockLoc {
            container_id: self.container_id().into(),
            block_src: self.in_file(LocalBlockSrc::ParBlock(block.to_ptr())),
        });
        let idx = self.arena_blocks().alloc(BlockInfo { ident, block_id });
        let src = self.in_file(LocalBlockSrc::ParBlock(block.to_ptr()));
        self.src_map_blocks().insert(src, idx);
        Some(idx)
    }
}
