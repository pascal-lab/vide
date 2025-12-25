use data_ty::DataTy;
use itertools::Itertools;
use la_arena::{Arena, Idx};
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
};

use super::literal::{Literal, lower_literal};
use crate::{
    db::InternDb,
    define_src,
    hir_def::{Ident, alloc_idx_and_src, literal::lower_integer_vector, lower_ident_opt},
    source_map::SourceMap,
};

pub mod data_ty;
pub mod declarator;
pub mod timing_control;

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
    // Assignments
    Assign(AssignOp),
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
    // `<=`
    NonBlockAssign,
    // `+=`
    AddAssign,
    // `-=`
    SubAssign,
    // `*=`
    MulAssign,
    // `/=`
    DivAssign,
    // `%=`
    ModAssign,
    // `&=`
    BitAndAssign,
    // `|=`
    BitOrAssign,
    // `^=`
    BitXorAssign,
    // `<<=`
    ShiftLeftAssign,
    // `>>=`
    ShiftRightAssign,
    // `<<<=`
    ArithShiftLeftAssign,
    // `>>>=`
    ArithShiftRightAssign,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum StreamOp {
    None,
    // >>
    Right,
    // <<
    Left,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assign {
    pub lhs: ExprId,
    pub rhs: ExprId,
    pub op: AssignOp,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub enum Expr {
    #[default]
    Missing,

    Binary {
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    Call {
        callee: ExprId,
        args: Box<[Arg]>,
    },
    Concat(Box<[ExprId]>),
    Cond {
        pred: ExprId,
        true_expr: ExprId,
        false_expr: ExprId,
    },
    Field {
        receiver: ExprId,
        field: Option<Ident>,
    },
    Ident(Ident),
    Literal(Literal),
    Cast {
        ty: DataTy,
        expr: ExprId,
    },
    SignedCast {
        signed: bool,
        expr: ExprId,
    },
    MinTypMax {
        min: ExprId,
        typ: ExprId,
        max: ExprId,
    },
    MultiConcat {
        concat: Box<[ExprId]>,
        rep: ExprId,
    },
    PostfixIncDec {
        op: IncDecOp,
        val: ExprId,
    },
    PrefixIncDec {
        op: IncDecOp,
        val: ExprId,
    },
    ElementSelect {
        receiver: ExprId,
        select: Option<Selector>,
    },
    Stream {
        op: StreamOp,
        slice: Option<ExprId>,
        concats: Box<[ExprId]>,
    },
    Unary {
        op: UnaryOp,
        expr: ExprId,
    },
}

pub type ExprId = Idx<Expr>;

define_src!(ExprSrc(ast::Expression));

impl Expr {
    pub fn to_assign(&self) -> Option<Assign> {
        match self {
            Expr::Binary { op, lhs, rhs } => {
                let op = match op {
                    BinaryOp::Assign(op) => *op,
                    _ => return None,
                };
                Some(Assign { lhs: *lhs, rhs: *rhs, op })
            }
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Arg {
    Named { name: Option<Ident>, expr: ExprId },
    Ordered(ExprId),
    Empty,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub enum Selector {
    Bit(ExprId),
    Range(ExprId, ExprId),
    Ascending(ExprId, ExprId),
    Descending(ExprId, ExprId),
}

pub(crate) trait LowerExpr {
    fn expr_ctx(&mut self) -> LowerExprCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_expr {
    ($ctx:ty $(,$data:ident, $src_map:ident)?) => {
        impl $crate::hir_def::expr::LowerExpr for $ctx {
            fn expr_ctx(&mut self) -> $crate::hir_def::expr::LowerExprCtx<'_> {
                $crate::hir_def::expr::LowerExprCtx {
                    db: self.db,
                    exprs: &mut self.$($data.)?exprs,
                    expr_srcs: &mut self.$($src_map.)?expr_srcs,
                }
            }
        }
    },
}

pub(crate) struct LowerExprCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,
}

impl LowerExprCtx<'_> {
    pub(crate) fn lower_expr_opt(&mut self, expr: Option<ast::Expression>) -> ExprId {
        if let Some(expr) = expr { self.lower_expr(expr) } else { self.alloc_missing() }
    }

    pub(crate) fn lower_expr(&mut self, expr: ast::Expression) -> ExprId {
        if let Some(hir_expr) = self.lower_expr_inner(expr) {
            alloc_idx_and_src! {
                hir_expr => self.exprs,
                expr => self.expr_srcs,
            }
        } else {
            self.alloc_missing()
        }
    }

    fn lower_expr_inner(&mut self, expr: ast::Expression) -> Option<Expr> {
        use ast::Expression::*;
        match expr {
            PrimaryExpression(primary) => self.lower_primary_expr(primary),
            BinaryExpression(binary_expr) => self.lower_binary_expr(binary_expr),
            Name(name) => self.lower_name(name),
            InvocationExpression(expr) => self.lower_invocation_expr(expr),
            PrefixUnaryExpression(expr) => self.lower_prefix_unary_expr(expr),
            ElementSelectExpression(expr) => self.lower_select_expr(expr),
            MinTypMaxExpression(expr) => self.lower_min_typ_max_expr(expr),
            MemberAccessExpression(expr) => self.lower_member_access_expr(expr),
            ConditionalExpression(expr) => self.lower_cond_expr(expr),
            CastExpression(expr) => self.lower_cast_expr(expr),
            SignedCastExpression(expr) => self.lower_cast_signed_expr(expr),
            PostfixUnaryExpression(expr) => self.lower_postfix_unary_expr(expr),
            _ => None,
        }
    }

    pub(crate) fn lower_assign(&mut self, expr: ast::Expression) -> Option<Assign> {
        self.lower_expr_inner(expr)?.to_assign()
    }

    fn lower_primary_expr(&mut self, expr: ast::PrimaryExpression) -> Option<Expr> {
        use ast::PrimaryExpression::*;
        match expr {
            LiteralExpression(lit) => lower_literal(lit).map(Expr::Literal),
            IntegerVectorExpression(int_vec) => lower_integer_vector(int_vec).map(Expr::Literal),
            MultipleConcatenationExpression(expr) => self.lower_multiple_concat_expr(expr),
            StreamingConcatenationExpression(expr) => self.lower_stream_concat_expr(expr),
            ConcatenationExpression(expr) => self.lower_concat_expr(expr),
            ParenthesizedExpression(expr) => self.lower_expr_inner(expr.expression()),
            _ => None,
        }
    }

    fn lower_member_access_expr(&mut self, expr: ast::MemberAccessExpression) -> Option<Expr> {
        let receiver = self.lower_expr(expr.left());
        let field = lower_ident_opt(expr.name());
        Some(Expr::Field { receiver, field })
    }

    fn lower_stream_concat_expr(
        &mut self,
        expr: ast::StreamingConcatenationExpression,
    ) -> Option<Expr> {
        let op = match expr.operator_token().map(|tok| tok.kind()) {
            None => StreamOp::None,
            Some(TokenKind::LEFT_SHIFT) => StreamOp::Left,
            Some(TokenKind::RIGHT_SHIFT) => StreamOp::Right,
            Some(_) => {
                unreachable!(
                    "lower_stream_concat_expr: {:?}",
                    expr.operator_token().unwrap().kind()
                )
            }
        };
        let slice = expr.slice_size().map(|size| self.lower_expr(size));

        // TODO: handle with-range
        let concats =
            expr.expressions().children().map(|expr| self.lower_expr(expr.expression())).collect();
        Some(Expr::Stream { op, slice, concats })
    }

    fn lower_name(&mut self, name: ast::Name) -> Option<Expr> {
        fn lower_ident_select(
            ctx: &mut LowerExprCtx,
            ident_select: ast::IdentifierSelectName,
        ) -> Option<Expr> {
            let mut expr =
                lower_ident_opt(ident_select.identifier()).map_or(Expr::Missing, Expr::Ident);

            let mut selectors = ident_select
                .selectors()
                .children()
                .filter_map(|sel| Some(ctx.lower_selector(sel.selector()?)))
                .collect_vec()
                .into_iter()
                .peekable();

            let src = ast::Expression::cast(ident_select.syntax()).unwrap().into();
            loop {
                match selectors.next() {
                    select @ Some(_) => {
                        let receiver = ctx.exprs.alloc(expr);
                        ctx.expr_srcs.insert(src, receiver);
                        expr = Expr::ElementSelect { receiver, select };
                    }
                    None => return Some(expr),
                }
            }
        }

        use ast::Name::*;
        match name {
            ast::Name::SystemName(ident) => {
                Some(lower_ident_opt(ident.system_identifier()).map_or(Expr::Missing, Expr::Ident))
            }
            ast::Name::IdentifierSelectName(ident_select) => lower_ident_select(self, ident_select),
            ast::Name::IdentifierName(ident) => {
                Some(lower_ident_opt(ident.identifier()).map_or(Expr::Missing, Expr::Ident))
            }
            ast::Name::ScopedName(scoped) => {
                let left = ast::Expression::cast(scoped.left().syntax()).unwrap();
                let receiver = self.lower_expr(left);

                match scoped.right() {
                    IdentifierName(ident) => {
                        let field = lower_ident_opt(ident.identifier());
                        Some(Expr::Field { receiver, field })
                    }
                    IdentifierSelectName(ident_select) => lower_ident_select(self, ident_select),
                    _ => unreachable!("lower_name: {:?}", scoped.right().syntax().kind()),
                }
            }
            _ => unimplemented!("lower_name: {:?}", name.syntax().kind()),
        }
    }

    fn lower_binary_expr(&mut self, expr: ast::BinaryExpression) -> Option<Expr> {
        let left = self.lower_expr(expr.left());
        let op = match expr.operator_token().unwrap().kind() {
            TokenKind::PLUS => BinaryOp::Add,
            TokenKind::MINUS => BinaryOp::Sub,
            TokenKind::STAR => BinaryOp::Mul,
            TokenKind::SLASH => BinaryOp::Div,
            TokenKind::PERCENT => BinaryOp::Mod,
            TokenKind::DOUBLE_STAR => BinaryOp::Pow,
            TokenKind::DOUBLE_EQUALS => BinaryOp::Eq,
            TokenKind::EXCLAMATION_EQUALS => BinaryOp::Neq,
            TokenKind::TRIPLE_EQUALS => BinaryOp::CaseEq,
            TokenKind::EXCLAMATION_DOUBLE_EQUALS => BinaryOp::CaseNeq,
            TokenKind::DOUBLE_EQUALS_QUESTION => BinaryOp::WildEq,
            TokenKind::EXCLAMATION_EQUALS_QUESTION => BinaryOp::WildNeq,
            TokenKind::GREATER_THAN => BinaryOp::Gt,
            TokenKind::GREATER_THAN_EQUALS => BinaryOp::Ge,
            TokenKind::LESS_THAN => BinaryOp::Lt,
            TokenKind::DOUBLE_AND => BinaryOp::LogAnd,
            TokenKind::DOUBLE_OR => BinaryOp::LogOr,
            TokenKind::RIGHT_SHIFT => BinaryOp::ShiftRight,
            TokenKind::LEFT_SHIFT => BinaryOp::ShiftLeft,
            TokenKind::TRIPLE_RIGHT_SHIFT => BinaryOp::ArithShiftRight,
            TokenKind::TRIPLE_LEFT_SHIFT => BinaryOp::ArithShiftLeft,
            TokenKind::AND => BinaryOp::BitAnd,
            TokenKind::OR => BinaryOp::BitOr,
            TokenKind::XOR => BinaryOp::BitXor,
            TokenKind::LESS_THAN_EQUALS => {
                if expr.syntax().kind() == SyntaxKind::NONBLOCKING_ASSIGNMENT_EXPRESSION {
                    BinaryOp::Assign(AssignOp::NonBlockAssign)
                } else {
                    BinaryOp::Le
                }
            }
            TokenKind::TILDE_XOR | TokenKind::XOR_TILDE => BinaryOp::BitXnor,
            TokenKind::EQUALS => BinaryOp::Assign(AssignOp::Assign),
            TokenKind::PLUS_EQUAL => BinaryOp::Assign(AssignOp::AddAssign),
            TokenKind::MINUS_EQUAL => BinaryOp::Assign(AssignOp::SubAssign),
            TokenKind::STAR_EQUAL => BinaryOp::Assign(AssignOp::MulAssign),
            TokenKind::SLASH_EQUAL => BinaryOp::Assign(AssignOp::DivAssign),
            TokenKind::PERCENT_EQUAL => BinaryOp::Assign(AssignOp::ModAssign),
            TokenKind::AND_EQUAL => BinaryOp::Assign(AssignOp::BitAndAssign),
            TokenKind::OR_EQUAL => BinaryOp::Assign(AssignOp::BitOrAssign),
            TokenKind::XOR_EQUAL => BinaryOp::Assign(AssignOp::BitXorAssign),
            TokenKind::LEFT_SHIFT_EQUAL => BinaryOp::Assign(AssignOp::ShiftLeftAssign),
            TokenKind::RIGHT_SHIFT_EQUAL => BinaryOp::Assign(AssignOp::ShiftRightAssign),
            TokenKind::TRIPLE_LEFT_SHIFT_EQUAL => BinaryOp::Assign(AssignOp::ArithShiftLeftAssign),
            TokenKind::TRIPLE_RIGHT_SHIFT_EQUAL => {
                BinaryOp::Assign(AssignOp::ArithShiftRightAssign)
            }
            _ => return None,
        };
        let right = self.lower_expr(expr.right());
        Some(Expr::Binary { op, lhs: left, rhs: right })
    }

    fn lower_prefix_unary_expr(&mut self, expr: ast::PrefixUnaryExpression) -> Option<Expr> {
        let val = self.lower_expr(expr.operand());
        let op = match expr.operator_token()?.kind() {
            TokenKind::PLUS => UnaryOp::Pos,
            TokenKind::MINUS => UnaryOp::Neg,
            TokenKind::EXCLAMATION => UnaryOp::LogNeg,
            TokenKind::TILDE => UnaryOp::BitNeg,
            TokenKind::AND => UnaryOp::ReducAnd,
            TokenKind::TILDE_AND => UnaryOp::ReducNand,
            TokenKind::OR => UnaryOp::ReducOr,
            TokenKind::TILDE_OR => UnaryOp::ReducNor,
            TokenKind::XOR => UnaryOp::ReducXor,
            TokenKind::TILDE_XOR | TokenKind::XOR_TILDE => UnaryOp::ReducXnor,
            TokenKind::DOUBLE_PLUS => {
                return Some(Expr::PrefixIncDec { op: IncDecOp::Inc, val });
            }
            TokenKind::DOUBLE_MINUS => {
                return Some(Expr::PrefixIncDec { op: IncDecOp::Dec, val });
            }
            _ => return None,
        };
        Some(Expr::Unary { op, expr: val })
    }

    fn lower_postfix_unary_expr(&mut self, expr: ast::PostfixUnaryExpression) -> Option<Expr> {
        let val = self.lower_expr(expr.operand());
        let op = match expr.operator_token()?.kind() {
            TokenKind::DOUBLE_PLUS => IncDecOp::Inc,
            TokenKind::DOUBLE_MINUS => IncDecOp::Dec,
            _ => return None,
        };
        Some(Expr::PostfixIncDec { op, val })
    }

    fn lower_cond_expr(&mut self, expr: ast::ConditionalExpression) -> Option<Expr> {
        // NOTE: We do not support patterns currently
        let cond_pred = expr.predicate().conditions().children().next().map(|pred| pred.expr());
        let pred = self.lower_expr_opt(cond_pred);
        let true_expr = self.lower_expr(expr.left());
        let false_expr = self.lower_expr(expr.right());
        Some(Expr::Cond { pred, true_expr, false_expr })
    }

    fn lower_concat_expr(&mut self, expr: ast::ConcatenationExpression) -> Option<Expr> {
        let concat = expr.expressions().children().map(|expr| self.lower_expr(expr)).collect();
        Some(Expr::Concat(concat))
    }

    fn lower_multiple_concat_expr(
        &mut self,
        expr: ast::MultipleConcatenationExpression,
    ) -> Option<Expr> {
        let rep = self.lower_expr(expr.expression());
        let concat = expr
            .concatenation()
            .expressions()
            .children()
            .map(|expr| self.lower_expr(expr))
            .collect();
        Some(Expr::MultiConcat { rep, concat })
    }

    fn lower_cast_expr(&mut self, expr: ast::CastExpression) -> Option<Expr> {
        let ty = self.lower_data_ty(expr.left().as_data_type()?);

        let right = ast::Expression::cast(expr.right().syntax()).unwrap();
        let expr = self.lower_expr(right);
        Some(Expr::Cast { ty, expr })
    }

    fn lower_cast_signed_expr(&mut self, expr: ast::SignedCastExpression) -> Option<Expr> {
        let signed = match expr.signing().unwrap().kind() {
            TokenKind::SIGNED_KEYWORD => true,
            TokenKind::UNSIGNED_KEYWORD => false,
            _ => unreachable!(),
        };

        let inner = ast::Expression::cast(expr.inner().syntax()).unwrap();
        let expr = self.lower_expr(inner);
        Some(Expr::SignedCast { signed, expr })
    }

    fn lower_min_typ_max_expr(&mut self, expr: ast::MinTypMaxExpression) -> Option<Expr> {
        let min = self.lower_expr(expr.min());
        let typ = self.lower_expr(expr.typ());
        let max = self.lower_expr(expr.max());
        Some(Expr::MinTypMax { min, typ, max })
    }

    fn lower_invocation_expr(&mut self, expr: ast::InvocationExpression) -> Option<Expr> {
        let callee = self.lower_expr(expr.left());
        let args =
            expr.arguments()?.parameters().children().map(|arg| self.lower_argument(arg)).collect();
        Some(Expr::Call { callee, args })
    }

    fn lower_argument(&mut self, arg: ast::Argument) -> Arg {
        use ast::Argument::*;
        match arg {
            NamedArgument(arg) => {
                let name = lower_ident_opt(arg.name());
                let expr = match arg.expr() {
                    Some(expr) => self.lower_property_expr(expr),
                    None => self.alloc_missing(),
                };
                Arg::Named { name, expr }
            }
            OrderedArgument(arg) => {
                let expr = self.lower_property_expr(arg.expr());
                Arg::Ordered(expr)
            }
            EmptyArgument(_) => Arg::Empty,
        }
    }

    fn lower_select_expr(&mut self, expr: ast::ElementSelectExpression) -> Option<Expr> {
        let receiver = self.lower_expr(expr.left());
        let select = expr.select().selector().map(|sel| self.lower_selector(sel));
        Some(Expr::ElementSelect { receiver, select })
    }

    pub(crate) fn lower_selector(&mut self, selector: ast::Selector) -> Selector {
        use ast::{RangeSelect::*, Selector::*};
        match selector {
            RangeSelect(range_sel) => {
                let left = self.lower_expr(range_sel.left());
                let right = self.lower_expr(range_sel.right());
                match range_sel {
                    AscendingRangeSelect(_) => Selector::Ascending(left, right),
                    DescendingRangeSelect(_) => Selector::Descending(left, right),
                    SimpleRangeSelect(_) => Selector::Range(left, right),
                }
            }
            BitSelect(bit_sel) => Selector::Bit(self.lower_expr(bit_sel.expr())),
        }
    }

    fn alloc_missing(&mut self) -> ExprId {
        self.exprs.alloc(Expr::Missing)
    }
}

impl LowerExprCtx<'_> {
    pub(crate) fn lower_property_expr(&mut self, expr: ast::PropertyExpr) -> ExprId {
        self.lower_property_expr_inner(expr).unwrap_or_else(|| self.alloc_missing())
    }

    pub(crate) fn lower_property_expr_inner(&mut self, expr: ast::PropertyExpr) -> Option<ExprId> {
        expr.as_simple_property_expr().and_then(|expr| self.lower_sequence_expr(expr.expr()))
    }

    pub(crate) fn lower_sequence_expr(&mut self, expr: ast::SequenceExpr) -> Option<ExprId> {
        expr.as_simple_sequence_expr().map(|expr| self.lower_expr(expr.expr()))
    }
}
