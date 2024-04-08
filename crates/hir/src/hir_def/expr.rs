use super::{literal::{Literal, LowerLiteral}, lower::Lower};
use crate::{
    hir_def::{data::DataType, Ident, InFile, SourceMap},
    try_match,
};
use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::ast::{self, ptr, AstNode};
use utils::{try_, try_or_default};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalExprSrc {
    Expr(ptr::ExpressionPtr),
    Primary(ptr::PrimaryPtr),
    ConstExpr(ptr::ConstantExpressionPtr),
    ConstPrimary(ptr::ConstantPrimaryPtr),
    ParamExpr(ptr::ParamExpressionPtr),
    ConstParamExpr(ptr::ConstantParamExpressionPtr),
    // NetLValue(ptr::NetLvaluePtr),
    // VarLValue(ptr::VariableLvaluePtr),
    // TODO: NoneRangeVarLValue(ptr::NonerangeVariableLvaluePtr),
}

pub type ExprSrc = InFile<LocalExprSrc>;

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum UnaryOp {
    // `+`
    Pos,
    // `-`
    Neg,
    // `!`
    LogNeg,
    // `~`
    BitNeg,
    // `&`
    ReducAnd,
    // `~&`
    ReducNand,
    // `|`
    ReducOr,
    // `~|`
    ReducNor,
    // `^`
    ReducXor,
    // `~^`, same as `^~`
    ReducXnor,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum BinaryOp {
    // Arithmetic operators
    // `+`
    Add,
    // `-`
    Sub,
    // `*`
    Mul,
    // `/`
    Div,
    // `%`
    Mod,
    // `**`
    Pow,
    // Equality operators
    // `==`
    Eq,
    // `!=`
    Neq,
    // `===`
    CaseEq,
    // `!==`
    CaseNeq,
    // `==?`
    WildEq,
    // `!=?`
    WildNeq,
    // Relational operators
    // `>`
    Gt,
    // `>=`
    Ge,
    // `<`
    Lt,
    // `<=`
    Le,
    // Logical operators
    // `&&`
    LogAnd,
    // `||`
    LogOr,
    // Shift operators
    // `>>`
    ShiftRight,
    // `<<`
    ShiftLeft,
    // `>>>`
    ArithShiftRight,
    // `<<<`
    ArithShiftLeft,
    // Bitwise operators
    // `&`
    BitAnd,
    // `|`
    BitOr,
    // `^`
    BitXor,
    // `~^`, same as `^~`
    BitXnor,
    // TODO: implication and equivalence
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum IncDecOp {
    // `++`
    Inc,
    // `--`
    Dec,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum AssignOp {
    // `=`
    Assign,
    // `+=`
    BinaryOpAssign(BinaryOp),
}

pub(crate) fn lower_assign_op(op: &ast::AssignmentOperator) -> Option<AssignOp> {
    try_match! {
        op.token_eq(), _ => Some(AssignOp::Assign),
        op.token_plus_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::Add)),
        op.token_minus_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::Sub)),
        op.token_star_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::Mul)),
        op.token_slash_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::Div)),
        op.token_percent_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::Mod)),
        op.token_and_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::BitAnd)),
        op.token_or_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::BitOr)),
        op.token_xor_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::BitXor)),
        op.token_lshift_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::ShiftLeft)),
        op.token_rshift_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::ShiftRight)),
        op.token_arith_lshift_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::ArithShiftLeft)),
        op.token_arith_rshift_eq(), _ => Some(AssignOp::BinaryOpAssign(BinaryOp::ArithShiftRight)),
        _ => None,
    }
}

pub(crate) fn lower_unary_op(unary_op: ast::UnaryOperator) -> UnaryOp {
    try_match! {
        unary_op.token_plus(), _ => UnaryOp::Pos,
        unary_op.token_minus(), _ => UnaryOp::Neg,
        unary_op.token_not(), _ => UnaryOp::LogNeg,
        unary_op.token_tilde(), _ => UnaryOp::BitNeg,
        unary_op.token_and(), _ => UnaryOp::ReducAnd,
        unary_op.token_tilde_and(), _ => UnaryOp::ReducNand,
        unary_op.token_or(), _ => UnaryOp::ReducOr,
        unary_op.token_tilde_or(), _ => UnaryOp::ReducNor,
        unary_op.token_xor(), _ => UnaryOp::ReducXor,
        unary_op.token_tilde_xor(), _ => UnaryOp::ReducXnor,
        _ => unreachable!(),
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum MinTypMaxExpr {
    MinTypMax { min: ExprId, typ: ExprId, max: ExprId },
    Expr(ExprId),
}

impl MinTypMaxExpr {
    pub fn get_min(&self) -> ExprId {
        match self {
            MinTypMaxExpr::MinTypMax { min, .. } => *min,
            MinTypMaxExpr::Expr(expr) => *expr,
        }
    }

    pub fn get_typ(&self) -> ExprId {
        match self {
            MinTypMaxExpr::MinTypMax { typ, .. } => *typ,
            MinTypMaxExpr::Expr(expr) => *expr,
        }
    }

    pub fn get_max(&self) -> ExprId {
        match self {
            MinTypMaxExpr::MinTypMax { max, .. } => *max,
            MinTypMaxExpr::Expr(expr) => *expr,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Expr {
    // TODO: Add more expressions
    // TODO: only used in params
    Missing,
    Unary { op: UnaryOp, expr: ExprId },
    Binary { op: BinaryOp, lhs: ExprId, rhs: ExprId },
    Cond { cond: ExprId, true_expr: ExprId, false_expr: ExprId },
    IncDec { op: IncDecOp, lv: LValue, is_post: bool },

    // Primary
    Literal(Literal),
    Concat { concat: Box<[ExprId]>, range: Option<Select> },
    MultiConcat { rep: ExprId, concat: Box<[ExprId]>, range: Option<Select> },
    Cast { data_type: DataType, expr: ExprId },
    MinTypMax(MinTypMaxExpr),
    Call { callee: Path, args: Box<[Arg]> },
    LValue(LValue),
    // This,
    // Dollar,
    // Null,
    // TODO: add more primary expressions
}

impl Default for Expr {
    fn default() -> Self {
        Expr::Missing
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Arg {
    Positional(ExprId),
    Named { name: Ident, expr: ExprId },
    Default { name: Option<Ident> },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Select {
    pub bit_selects: Box<[ExprId]>,
    pub part_select: Option<PartSelectExpr>,
}

impl Select {
    pub fn traverse(self) -> Option<Select> {
        if self.bit_selects.is_empty() && self.part_select.is_none() {
            return None;
        }

        Some(self)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PartSelectExpr {
    Indexed { is_add: bool, index: ExprId, offset: ExprId },
    Range { lsb: ExprId, msb: ExprId },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path(Box<[Ident]>);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LValue {
    path: Path,
    select: Option<Select>,
}

pub type ExprId = Idx<Expr>;

macro_rules! map_or_missing {
    ($self:ident, $item:expr, $f:ident) => {
        match $item {
            Some(x) => $self.$f(&x),
            None => $self.alloc_missing(),
        }
    };
}

pub(crate) trait LowerExpr: LowerLiteral + Lower {
    fn arena_expr(&mut self) -> &mut Arena<Expr>;

    fn src_map_expr(&mut self) -> &mut SourceMap<ExprSrc, Expr>;

    fn alloc_missing(&mut self) -> ExprId {
        let expr = Expr::Missing;
        self.arena_expr().alloc(expr)
    }

    fn lower_systf_identifier(&self, ident: &ast::SystemTfIdentifier) -> Option<Ident> {
        // TODO: will to_text be too slow?
        ident.to_text(self.file_text()).map(|s| s.into())
    }

    fn lower_net_lvalue(&mut self, netlv: &ast::NetLvalue) -> Option<LValue> {
        let path = netlv
            .identifiers()
            .map(|ident| self.lower_ident(&ident))
            .collect::<Option<Box<[_]>>>()?;

        let select = if let Some(select) = netlv.constant_select() {
            Some(self.lower_const_select(&select)?)
        } else {
            None
        };

        Some(LValue { path: Path(path), select })
    }

    fn lower_var_lvalue(&mut self, varlv: &ast::VariableLvalue) -> Option<LValue> {
        let path = varlv
            .identifiers()
            .map(|ident| self.lower_ident(&ident))
            .collect::<Option<Box<[_]>>>()?;

        let select = if let Some(select) = varlv.select() {
            Some(self.lower_select(&select)?)
        } else {
            None
        };

        Some(LValue { path: Path(path), select })
    }

    fn lower_const_select(&mut self, select: &ast::ConstantSelect) -> Option<Select> {
        let bit_selects = select
            .constant_expressions()
            .map(|expr| self.lower_const_expr(&expr))
            .collect::<Box<[_]>>();

        let part_select = try_match!(
            select.constant_part_select_range(), part_select => {
                try_match!(
                    part_select.constant_range(), const_range => {
                        let mut exprs = const_range.constant_expressions();
                        let msb = map_or_missing!(self, exprs.next(), lower_const_expr);
                        let lsb = map_or_missing!(self, exprs.next(), lower_const_expr);
                        Some(PartSelectExpr::Range { lsb, msb })
                    },
                    part_select.constant_indexed_range(), indexed_range => {
                        let mut exprs = indexed_range.constant_expressions();
                        let index = map_or_missing!(self, exprs.next(), lower_const_expr);
                        let offset = map_or_missing!(self, exprs.next(), lower_const_expr);
                        let is_add = try_match!(
                            indexed_range.token_plus_colon(), _ => true,
                            indexed_range.token_minus_colon(), _ => false,
                            _ => return None,
                        );
                        Some(PartSelectExpr::Indexed { is_add, index, offset })
                    },
                    _ => None,
                )
            },
            _ => None,
        );

        let range = Select { bit_selects, part_select };
        Some(range)
    }

    fn lower_const_part_select_range(
        &mut self,
        part_select: &ast::ConstantPartSelectRange,
    )-> Option<PartSelectExpr> {
        try_match! {
            part_select.constant_range(), const_range => {
                let mut exprs = const_range.constant_expressions();
                let msb = map_or_missing!(self, exprs.next(), lower_const_expr);
                let lsb = map_or_missing!(self, exprs.next(), lower_const_expr);
                Some(PartSelectExpr::Range { lsb, msb })
            },
            part_select.constant_indexed_range(), indexed_range => {
                let mut exprs = indexed_range.constant_expressions();
                let index = map_or_missing!(self, exprs.next(), lower_const_expr);
                let offset = map_or_missing!(self, exprs.next(), lower_const_expr);
                let is_add = try_match!(
                    indexed_range.token_plus_colon(), _ => true,
                    indexed_range.token_minus_colon(), _ => false,
                    _ => return None,
                );
                Some(PartSelectExpr::Indexed { is_add, index, offset })
            },
            _ => None,
        }
    }

    fn lower_part_select_range(
        &mut self,
        part_select: &ast::PartSelectRange,
    ) -> Option<PartSelectExpr> {
        try_match! {
            part_select.constant_range(), const_range => {
                let mut exprs = const_range.constant_expressions();
                let msb = map_or_missing!(self, exprs.next(), lower_const_expr);
                let lsb = map_or_missing!(self, exprs.next(), lower_const_expr);
                Some(PartSelectExpr::Range { lsb, msb })
            },
            part_select.indexed_range(), indexed_range => {
                let index = map_or_missing!(self, indexed_range.expression(), lower_expr);
                let offset = map_or_missing!(self, indexed_range.constant_expression(), lower_const_expr);
                let is_add = try_match!(
                    indexed_range.token_plus_colon(), _ => true,
                    indexed_range.token_minus_colon(), _ => false,
                    _ => return None,
                );
                Some(PartSelectExpr::Indexed { is_add, index, offset })
            },
            _ => None,
        }
    }

    fn lower_select(&mut self, select: &ast::Select) -> Option<Select> {
        // TODO: optimize this
        let bit_selects = select
            .bit_selects()
            .flat_map(|sel| {
                sel.expressions()
                    .map(|expr| self.lower_expr(&expr))
                    .collect::<Vec<_>>()
                    .into_iter()
            })
            .collect::<Box<[_]>>();

        let part_select = try_match! {
            select.part_select_range(), part_select => self.lower_part_select_range(&part_select),
            _ => None,
        };

        let range = Select { bit_selects, part_select };
        Some(range)
    }

    fn lower_param_expr(&mut self, param_expr: &ast::ParamExpression) -> Option<ExprId> {
        let expr = try_match! {
            param_expr.mintypmax_expression(), min_typ_max => {
                let min_typ_max = self.lower_min_typ_max_expr(&min_typ_max);
                Expr::MinTypMax(min_typ_max)
            },
            _ => {
                Expr::Missing
                // TODO: ("Unsupported");
            }
        };

        let expr_id = self.arena_expr().alloc(expr);
        let src = self.in_file(LocalExprSrc::ParamExpr(param_expr.to_ptr()));
        self.src_map_expr().insert(src, expr_id);
        Some(expr_id)
    }

    fn lower_const_param_expr(
        &mut self,
        param_expr: &ast::ConstantParamExpression,
    ) -> Option<ExprId> {
        let expr = try_match! {
            param_expr.constant_mintypmax_expression(), min_typ_max => {
                let min_typ_max = self.lower_const_min_typ_max_expr(&min_typ_max);
                Expr::MinTypMax(min_typ_max)
            },
            _ => {
                Expr::Missing
                // TODO: ("Unsupported");
            }
        };

        let expr_id = self.arena_expr().alloc(expr);
        let src = self.in_file(LocalExprSrc::ConstParamExpr(param_expr.to_ptr()));
        self.src_map_expr().insert(src, expr_id);
        Some(expr_id)
    }

    fn lower_const_range_expr(&mut self, range: &ast::ConstantRangeExpression) -> Option<Select> {
        try_match! {
            range.constant_expression(), expr => {
                let expr_id = self.lower_const_expr(&expr);
                Select {
                    bit_selects: Box::new([expr_id]),
                    part_select: None,
                }.traverse()
            },
            range.constant_part_select_range(), part_select => {
                let part_select = self.lower_const_part_select_range(&part_select)?;
                Select {
                    bit_selects: Box::new([]),
                    part_select: Some(part_select),
                }.traverse()
            },
            _ => None,
        }
    }

    fn lower_range_expr(&mut self, range: &ast::RangeExpression) -> Option<Select> {
        try_match! {
            range.expression(), expr => {
                let expr_id = self.lower_expr(&expr);
                Select {
                    bit_selects: Box::new([expr_id]),
                    part_select: None,
                }.traverse()
            },
            range.part_select_range(), part_select => {
                let part_select = self.lower_part_select_range(&part_select)?;
                Select {
                    bit_selects: Box::new([]),
                    part_select: Some(part_select),
                }.traverse()
            },
            _ => None,
        }
    }

    fn lower_list_of_args(&mut self, arg_list: &ast::ListOfArgumentsParent) -> Box<[Arg]> {
        let mut cursor = arg_list.syntax().walk();
        if !cursor.goto_first_child() {
            return Box::new([]);
        }
        let mut args = Vec::new();
        loop {
            if cursor.node().kind_id() == syntax::syntax_kind::IDENTIFIER {
                let ident = ast::Identifier::cast(cursor.node()).unwrap();
                let name = self.lower_ident(&ident).unwrap();
                cursor.goto_next_sibling();
                if cursor.node().kind_id() == syntax::syntax_kind::EXPRESSION {
                    let expr =
                        map_or_missing!(self, ast::Expression::cast(cursor.node()), lower_expr);
                    args.push(Arg::Named { name, expr });
                } else {
                    args.push(Arg::Default { name: Some(name) });
                }
                cursor.goto_next_sibling();
                if cursor.node().kind_id() == syntax::syntax_kind::TOKEN_COMMA {
                    cursor.goto_next_sibling();
                } else {
                    break;
                }
            } else if cursor.node().kind_id() == syntax::syntax_kind::EXPRESSION {
                let expr = map_or_missing!(self, ast::Expression::cast(cursor.node()), lower_expr);
                args.push(Arg::Positional(expr));
                cursor.goto_next_sibling();
                if cursor.node().kind_id() == syntax::syntax_kind::TOKEN_COMMA {
                    cursor.goto_next_sibling();
                } else {
                    break;
                }
            } else if cursor.node().kind_id() == syntax::syntax_kind::TOKEN_COMMA {
                args.push(Arg::Default { name: None });
                cursor.goto_next_sibling();
            } else {
                break;
            }
        }
        args.into_boxed_slice()
    }

    fn lower_const_primary_expr(&mut self, primary: &ast::ConstantPrimary) -> ExprId {
        let expr = try_match! {
            primary.primary_literal(), literal => {
                self.lower_literal(&literal).map(|lit| Expr::Literal(lit)).unwrap_or_default()
            },
            primary.constant_concatenation(), concat => {
                let concat = concat
                    .constant_expressions()
                    .map(|expr| self.lower_const_expr(&expr))
                    .collect();
                let range = primary
                    .constant_range_expression()
                    .and_then(|range| self.lower_const_range_expr(&range));
                Expr::Concat { concat, range }
            },
            primary.constant_multiple_concatenation(), mc => {
                let rep = map_or_missing!(self, mc.constant_expression(), lower_const_expr);
                let concat = try_or_default! {
                    mc.constant_concatenation()?
                        .constant_expressions()
                        .map(|expr| self.lower_const_expr(&expr))
                        .collect()
                };
                let range = primary.constant_range_expression().and_then(|range| self.lower_const_range_expr(&range));
                Expr::MultiConcat { rep, concat, range }
            },
            primary.constant_mintypmax_expression(), mintypmax => {
                Expr::MinTypMax(self.lower_const_min_typ_max_expr(&mintypmax))
            },
            _ => {
                Expr::Missing
            }
        };

        let expr_id = self.arena_expr().alloc(expr);
        let src = self.in_file(LocalExprSrc::ConstPrimary(primary.to_ptr()));
        self.src_map_expr().insert(src, expr_id);
        expr_id
    }

    fn lower_primary_expr(&mut self, primary: &ast::Primary) -> ExprId {
        let expr = try_match! {
            primary.primary_literal(), literal => {
                self.lower_literal(&literal)
                    .map(|lit| Expr::Literal(lit))
                    .unwrap_or_default()
            },
            primary.concatenation(), concat => {
                let concat = concat
                    .expressions()
                    .map(|expr| self.lower_expr(&expr))
                    .collect();
                let range = primary.range_expression()
                    .and_then(|range| self.lower_range_expr(&range));
                Expr::Concat { concat, range }
            },
            primary.multiple_concatenation(), mc => {
                let rep = map_or_missing!(self, mc.expression(), lower_expr);
                let concat = try_or_default! {
                    mc.concatenation()?
                        .expressions()
                        .map(|expr| self.lower_expr(&expr))
                        .collect()
                };
                let range = primary.range_expression()
                    .and_then(|range| self.lower_range_expr(&range));
                Expr::MultiConcat { rep, concat, range }
            },
            primary.function_subroutine_call(), call => {
                // TODO: lower const bit select
                try_or_default! {
                    let call = call.subroutine_call()?;
                    try_match! {
                        call.tf_call(), tf_call => {
                            let path = tf_call
                                .identifiers()
                                .map(|ident| self.lower_ident(&ident))
                                .collect::<Option<Box<[_]>>>()?;
                            let args = tf_call
                                .list_of_arguments_parent()
                                .map_or_else(
                                    || Box::new([]) as Box<[_]>,
                                    |arg_list| self.lower_list_of_args(&arg_list)
                                );
                            Expr::Call { callee: Path(path), args }
                        },
                        call.system_tf_call(), sys_tf_call => {
                            let path = sys_tf_call
                                .system_tf_identifier()
                                .and_then(|ident| self.lower_systf_identifier(&ident))?;
                            let args = sys_tf_call
                                .list_of_arguments_parent()
                                .map_or_else::<Box<[Arg]>, _, _>(
                                    || Box::new([]),
                                    |arg_list| self.lower_list_of_args(&arg_list)
                                );
                            Expr::Call { callee: Path(Box::new([path])), args }
                        },
                        _ => Expr::Missing,
                    }
                }
            },
            primary.mintypmax_expression(), mintypmax => {
                Expr::MinTypMax(self.lower_min_typ_max_expr(&mintypmax))
            },
            primary.cast_(), cast => {
                let expr = map_or_missing!(self, cast.expression(), lower_expr);
                let ty = cast.casting_type();
                // todo!("casting type");
                Expr::Missing
            },
            _ => {
                if primary.identifiers().count() != 0 {
                    try_or_default! {
                        let path = primary.identifiers()
                            .map(|ident| self.lower_ident(&ident))
                            .collect::<Option<Box<[_]>>>()
                            .map(Path)?;

                        let select = if let Some(select) = primary.select() {
                            Some(self.lower_select(&select)?)
                        } else {
                            None
                        };

                        Expr::LValue(LValue { path, select })
                    }
                } else {
                    Expr::Missing
                }
            }
        };

        let expr_id = self.arena_expr().alloc(expr);
        let src = self.in_file(LocalExprSrc::Primary(primary.to_ptr()));
        self.src_map_expr().insert(src, expr_id);
        expr_id
    }

    // TODO: Should we stored it in src map?
    fn lower_const_min_typ_max_expr(
        &mut self,
        min_typ_max_expr: &ast::ConstantMintypmaxExpression,
    ) -> MinTypMaxExpr {
        let mut exprs = min_typ_max_expr
            .constant_expressions()
            .map(|expr| self.lower_const_expr(&expr))
            .collect::<SmallVec<[_; 3]>>();

        if exprs.len() == 1 {
            MinTypMaxExpr::Expr(exprs.pop().unwrap())
        } else if exprs.len() == 3 {
            let min = exprs.pop().unwrap();
            let typ = exprs.pop().unwrap();
            let max = exprs.pop().unwrap();
            MinTypMaxExpr::MinTypMax { min, typ, max }
        } else {
            unreachable!("Invalid number of expressions in min-typ-max expression")
        }
    }

    fn lower_min_typ_max_expr(
        &mut self,
        min_typ_max_expr: &ast::MintypmaxExpression,
    ) -> MinTypMaxExpr {
        let mut exprs = min_typ_max_expr
            .expressions()
            .map(|expr| self.lower_expr(&expr))
            .collect::<SmallVec<[_; 3]>>();

        if exprs.len() == 1 {
            MinTypMaxExpr::Expr(exprs.pop().unwrap())
        } else if exprs.len() == 3 {
            let min = exprs.pop().unwrap();
            let typ = exprs.pop().unwrap();
            let max = exprs.pop().unwrap();
            MinTypMaxExpr::MinTypMax { min, typ, max }
        } else {
            unreachable!("Invalid number of expressions in min-typ-max expression")
        }
    }

    fn lower_cond_predicate(&mut self, pred: &ast::CondPredicate) -> ExprId {
        // TODO: We do not support patterns
        try_! {
            self.lower_expr(&pred.expression_or_cond_patterns().next()?.expression()?)
        }
        .unwrap_or_else(|| self.alloc_missing())
    }

    fn lower_const_binary(
        &mut self,
        bin_expr: &ast::ConstantExpression,
        bin_op: BinaryOp,
    ) -> ExprId {
        let mut exprs = bin_expr.constant_expressions();
        let lhs = map_or_missing!(self, exprs.next(), lower_const_expr);
        let rhs = map_or_missing!(self, exprs.next(), lower_const_expr);
        let expr = Expr::Binary { op: bin_op, lhs, rhs };

        let expr_id = self.arena_expr().alloc(expr);
        let src = self.in_file(LocalExprSrc::ConstExpr(bin_expr.to_ptr()));
        self.src_map_expr().insert(src, expr_id);
        expr_id
    }

    fn lower_binary(&mut self, bin_expr: &ast::Expression, bin_op: BinaryOp) -> ExprId {
        let mut exprs = bin_expr.expressions();
        let lhs = map_or_missing!(self, exprs.next(), lower_expr);
        let rhs = map_or_missing!(self, exprs.next(), lower_expr);
        let expr = Expr::Binary { op: bin_op, lhs, rhs };

        let expr_id = self.arena_expr().alloc(expr);
        let src = self.in_file(LocalExprSrc::Expr(bin_expr.to_ptr()));
        self.src_map_expr().insert(src, expr_id);
        expr_id
    }

    fn lower_const_expr(&mut self, expr: &ast::ConstantExpression) -> ExprId {
        try_match! {
            expr.unary_operator(), unary_op => {
                let op = lower_unary_op(unary_op);
                let primary = map_or_missing!(self, expr.constant_primary(), lower_const_primary_expr);
                let expr_id = self.arena_expr().alloc(Expr::Unary { op, expr: primary });
                let src = self.in_file(LocalExprSrc::ConstExpr(expr.to_ptr()));
                self.src_map_expr().insert(src, expr_id);
                expr_id
            },
            expr.token_plus(), _ => self.lower_const_binary(&expr, BinaryOp::Add),
            expr.token_minus(), _ => self.lower_const_binary(&expr, BinaryOp::Sub),
            expr.token_star(), _ => self.lower_const_binary(&expr, BinaryOp::Mul),
            expr.token_slash(), _ => self.lower_const_binary(&expr, BinaryOp::Div),
            expr.token_percent(), _ => self.lower_const_binary(&expr, BinaryOp::Mod),
            expr.token_star_star(), _ => self.lower_const_binary(&expr, BinaryOp::Pow),
            expr.token_eq_eq(), _ => self.lower_const_binary(&expr, BinaryOp::Eq),
            expr.token_not_eq(), _ => self.lower_const_binary(&expr, BinaryOp::Neq),
            expr.token_eq_eq_eq(), _ => self.lower_const_binary(&expr, BinaryOp::CaseEq),
            expr.token_not_eq_eq(), _ => self.lower_const_binary(&expr, BinaryOp::CaseNeq),
            expr.token_eq_eq_question(), _ => self.lower_const_binary(&expr, BinaryOp::WildEq),
            expr.token_not_eq_question(), _ => self.lower_const_binary(&expr, BinaryOp::WildNeq),
            expr.token_greater(), _ => self.lower_const_binary(&expr, BinaryOp::Gt),
            expr.token_greater_eq(), _ => self.lower_const_binary(&expr, BinaryOp::Ge),
            expr.token_less(), _ => self.lower_const_binary(&expr, BinaryOp::Lt),
            expr.token_less_eq(), _ => self.lower_const_binary(&expr, BinaryOp::Le),
            expr.token_and_and(), _ => self.lower_const_binary(&expr, BinaryOp::LogAnd),
            expr.token_or_or(), _ => self.lower_const_binary(&expr, BinaryOp::LogOr),
            expr.token_rshift(), _ => self.lower_const_binary(&expr, BinaryOp::ShiftRight),
            expr.token_lshift(), _ => self.lower_const_binary(&expr, BinaryOp::ShiftLeft),
            expr.token_arith_rshift(), _ => self.lower_const_binary(&expr, BinaryOp::ArithShiftRight),
            expr.token_arith_lshift(), _ => self.lower_const_binary(&expr, BinaryOp::ArithShiftLeft),
            expr.token_and(), _ => self.lower_const_binary(&expr, BinaryOp::BitAnd),
            expr.token_or(), _ => self.lower_const_binary(&expr, BinaryOp::BitOr),
            expr.token_xor(), _ => self.lower_const_binary(&expr, BinaryOp::BitXor),
            expr.token_tilde_xor(), _ => self.lower_const_binary(&expr, BinaryOp::BitXnor),
            expr.token_xor_tilde(), _ => self.lower_const_binary(&expr, BinaryOp::BitXnor),
            expr.constant_primary(), primary => self.lower_const_primary_expr(&primary),
            _ => self.alloc_missing(),
        }
    }

    fn lower_expr(&mut self, expr: &ast::Expression) -> ExprId {
        try_match!(
            expr.unary_operator(), unary_op => {
                let op = lower_unary_op(unary_op);
                let primary = map_or_missing!(self, expr.primary(), lower_primary_expr);
                let expr_id = self.arena_expr().alloc(Expr::Unary { op, expr: primary });
                let src = self.in_file(LocalExprSrc::Expr(expr.to_ptr()));
                self.src_map_expr().insert(src, expr_id);
                expr_id
            },
            expr.inc_or_dec_expression(), inc_or_dec => {
                let is_inc = try_match!(
                    inc_or_dec.inc_or_dec_operator(), inc_op => {
                        try_match! {
                            inc_op.token_plus_plus(), _ => true,
                            inc_op.token_minus_minus(), _ => false,
                            _ => return self.alloc_missing(),
                        }
                    },
                    _ => return self.alloc_missing(),
                );
                let is_post = {
                    let mut cursor = inc_or_dec.syntax().walk();
                    if !cursor.goto_first_child() {
                        return self.alloc_missing();
                    }
                    loop {
                        if ast::VariableLvalue::can_cast(cursor.node().kind_id()) {
                            break true;
                        } else if ast::IncOrDecOperator::can_cast(cursor.node().kind_id()) {
                            break false;
                        } else if !cursor.goto_next_sibling() {
                            return self.alloc_missing();
                        }
                    }
                };
                let Some(lv) = try_!(self.lower_var_lvalue(&inc_or_dec.variable_lvalue()?)?) else {
                    return self.alloc_missing();
                };
                let expr_id = self.arena_expr().alloc(Expr::IncDec {
                    op: if is_inc { IncDecOp::Inc } else { IncDecOp::Dec },
                    lv,
                    is_post
                });
                let src = self.in_file(LocalExprSrc::Expr(expr.to_ptr()));
                self.src_map_expr().insert(src, expr_id);
                expr_id
            },
            expr.operator_assignment(), op_assign => {
                return self.alloc_missing();
                todo!("Unsupported")
            },
            expr.token_lparen(), _ => {
                if expr.token_rparen().is_none() {
                    return self.alloc_missing();
                }
                map_or_missing!(self, expr.expressions().next(), lower_expr)
            },
            expr.token_plus(), _ => self.lower_binary(&expr, BinaryOp::Add),
            expr.token_minus(), _ => self.lower_binary(&expr, BinaryOp::Sub),
            expr.token_star(), _ => self.lower_binary(&expr, BinaryOp::Mul),
            expr.token_slash(), _ => self.lower_binary(&expr, BinaryOp::Div),
            expr.token_percent(), _ => self.lower_binary(&expr, BinaryOp::Mod),
            expr.token_star_star(), _ => self.lower_binary(&expr, BinaryOp::Pow),
            expr.token_eq_eq(), _ => self.lower_binary(&expr, BinaryOp::Eq),
            expr.token_not_eq(), _ => self.lower_binary(&expr, BinaryOp::Neq),
            expr.token_eq_eq_eq(), _ => self.lower_binary(&expr, BinaryOp::CaseEq),
            expr.token_not_eq_eq(), _ => self.lower_binary(&expr, BinaryOp::CaseNeq),
            expr.token_eq_eq_question(), _ => self.lower_binary(&expr, BinaryOp::WildEq),
            expr.token_not_eq_question(), _ => self.lower_binary(&expr, BinaryOp::WildNeq),
            expr.token_greater(), _ => self.lower_binary(&expr, BinaryOp::Gt),
            expr.token_greater_eq(), _ => self.lower_binary(&expr, BinaryOp::Ge),
            expr.token_less(), _ => self.lower_binary(&expr, BinaryOp::Lt),
            expr.token_less_eq(), _ => self.lower_binary(&expr, BinaryOp::Le),
            expr.token_and_and(), _ => self.lower_binary(&expr, BinaryOp::LogAnd),
            expr.token_or_or(), _ => self.lower_binary(&expr, BinaryOp::LogOr),
            expr.token_rshift(), _ => self.lower_binary(&expr, BinaryOp::ShiftRight),
            expr.token_lshift(), _ => self.lower_binary(&expr, BinaryOp::ShiftLeft),
            expr.token_arith_rshift(), _ => self.lower_binary(&expr, BinaryOp::ArithShiftRight),
            expr.token_arith_lshift(), _ => self.lower_binary(&expr, BinaryOp::ArithShiftLeft),
            expr.token_and(), _ => self.lower_binary(&expr, BinaryOp::BitAnd),
            expr.token_or(), _ => self.lower_binary(&expr, BinaryOp::BitOr),
            expr.token_xor(), _ => self.lower_binary(&expr, BinaryOp::BitXor),
            expr.token_tilde_xor(), _ => self.lower_binary(&expr, BinaryOp::BitXnor),
            expr.token_xor_tilde(), _ => self.lower_binary(&expr, BinaryOp::BitXnor),
            expr.conditional_expression(), cond_expr => {
                let cond = map_or_missing!(self, cond_expr.cond_predicate(), lower_cond_predicate);

                let mut exprs = cond_expr.expressions();
                let true_expr = map_or_missing!(self, exprs.next(), lower_expr);
                let false_expr = map_or_missing!(self, exprs.next(), lower_expr);

                let cond_expr = Expr::Cond { cond, true_expr, false_expr };
                let expr_id = self.arena_expr().alloc(cond_expr);

                let src = self.in_file(LocalExprSrc::Expr(expr.to_ptr()));
                self.src_map_expr().insert(src, expr_id);
                expr_id
            },
            expr.primary(), primary => self.lower_primary_expr(&primary),
            _ => self.alloc_missing(),
        )
    }
}
