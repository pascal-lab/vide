use data_ty::DataTy;
use itertools::Itertools;
use la_arena::Idx;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
};
use timing_control::TimingControl;

use super::literal::{Literal, lower_literal};
use crate::{
    Ident, alloc_with_source_entry,
    literal::lower_integer_vector,
    lower::{LoweringCtx, LoweringStore},
    lower_ident, lower_ident_opt,
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
pub struct StreamRange {
    pub selector: Option<Selector>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct StreamExpr {
    pub expr: ExprId,
    pub with_range: Option<StreamRange>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Assign {
    pub lhs: ExprId,
    pub rhs: ExprId,
    pub op: AssignOp,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AssignmentPattern {
    Simple(Box<[ExprId]>),
    Structured(Box<[AssignmentPatternItem]>),
    Replicated { count: ExprId, items: Box<[ExprId]> },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum AssignmentPatternItem {
    KeyValue { key: ExprId, value: ExprId },
    Default { value: ExprId },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum InsideRange {
    Expr(ExprId),
    Range { left: ExprId, right: ExprId },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct SequenceRepetition {
    pub op: TokenKind,
    pub selector: Option<Selector>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DelayedSequenceElement {
    pub delay: Option<ExprId>,
    pub range: Option<Selector>,
    pub expr: ExprId,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum SequenceExpr {
    Simple { expr: ExprId, repetition: Option<SequenceRepetition> },
    Binary { left: ExprId, op: TokenKind, right: ExprId },
    Delayed { first: Option<ExprId>, elements: Box<[DelayedSequenceElement]> },
    Event(timing_control::EventExprId),
    Clocking { event: TimingControl, expr: ExprId },
    FirstMatch { expr: ExprId },
    Parenthesized { expr: ExprId, matches: Box<[ExprId]>, repetition: Option<SequenceRepetition> },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PropertyCaseItem {
    Default { expr: ExprId },
    Standard { expressions: Box<[ExprId]>, expr: ExprId },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PropertyExpr {
    Parenthesized { expr: ExprId, matches: Box<[ExprId]> },
    Simple(ExprId),
    Binary { left: ExprId, op: TokenKind, right: ExprId },
    Conditional { condition: ExprId, expr: ExprId, else_expr: Option<ExprId> },
    Unary { op: TokenKind, expr: ExprId },
    UnarySelect { op: TokenKind, selector: Option<Selector>, expr: ExprId },
    Clocking { event: TimingControl, expr: Option<ExprId> },
    StrongWeak { strong: bool, expr: ExprId },
    AcceptOn { condition: ExprId, expr: ExprId },
    Case { expr: ExprId, items: Box<[PropertyCaseItem]> },
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub enum Expr {
    #[default]
    Missing,
    Error(SyntaxKind),
    Unsupported(SyntaxKind),

    Binary {
        op: BinaryOp,
        lhs: ExprId,
        rhs: ExprId,
    },
    AssignmentPattern {
        ty: Option<DataTy>,
        pattern: AssignmentPattern,
    },
    Inside {
        expr: ExprId,
        ranges: Box<[InsideRange]>,
    },
    Dist {
        expr: ExprId,
        distribution: crate::constraint::DistConstraintList,
    },
    TimingControl {
        control: TimingControl,
        expr: ExprId,
    },
    Sequence(SequenceExpr),
    Property(PropertyExpr),
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
        concats: Box<[StreamExpr]>,
    },
    Unary {
        op: UnaryOp,
        expr: ExprId,
    },
}

pub type ExprId = Idx<Expr>;

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

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn lower_expr_opt(&mut self, expr: Option<ast::Expression>) -> ExprId {
        if let Some(expr) = expr { self.lower_expr(expr) } else { self.alloc_missing_expr() }
    }

    pub(crate) fn lower_expr(&mut self, expr: ast::Expression) -> ExprId {
        let syntax_kind = expr.syntax().kind();
        let hir_expr = self.lower_expr_inner(expr).unwrap_or(Expr::Error(syntax_kind));
        match &hir_expr {
            Expr::Error(_) => {
                self.report_invalid(expr.syntax(), "invalid expression");
            }
            Expr::Unsupported(_) => {
                self.report_unsupported(expr.syntax(), "unsupported expression");
            }
            _ => {}
        }
        let source = self.source_id(expr.syntax());
        let (expressions, sources) = self.expressions();
        alloc_with_source_entry(expressions, sources, hir_expr, source)
    }

    fn alloc_lowered_expr(&mut self, expr: Expr, syntax: syntax::SyntaxNode<'_>) -> ExprId {
        let source = self.source_id(syntax);
        let (expressions, sources) = self.expressions();
        alloc_with_source_entry(expressions, sources, expr, source)
    }

    pub(crate) fn lower_sequence_expr(&mut self, expr: ast::SequenceExpr) -> ExprId {
        let lowered = match expr {
            ast::SequenceExpr::SimpleSequenceExpr(expr) => SequenceExpr::Simple {
                expr: self.lower_expr(expr.expr()),
                repetition: expr
                    .repetition()
                    .map(|repetition| self.lower_sequence_repetition(repetition)),
            },
            ast::SequenceExpr::BinarySequenceExpr(expr) => SequenceExpr::Binary {
                left: self.lower_sequence_expr(expr.left()),
                op: expr.op().map(|token| token.kind()).unwrap_or(TokenKind::UNKNOWN),
                right: self.lower_sequence_expr(expr.right()),
            },
            ast::SequenceExpr::DelayedSequenceExpr(expr) => SequenceExpr::Delayed {
                first: expr.first().map(|first| self.lower_sequence_expr(first)),
                elements: expr
                    .elements()
                    .children()
                    .map(|element| DelayedSequenceElement {
                        delay: element.delay_val().map(|delay| self.lower_expr(delay)),
                        range: element.range().map(|range| self.lower_selector(range)),
                        expr: self.lower_sequence_expr(element.expr()),
                    })
                    .collect(),
            },
            ast::SequenceExpr::EventExpression(expr) => {
                SequenceExpr::Event(self.lower_event_expr(expr))
            }
            ast::SequenceExpr::ClockingSequenceExpr(expr) => SequenceExpr::Clocking {
                event: self.lower_timing_control(expr.event()),
                expr: self.lower_sequence_expr(expr.expr()),
            },
            ast::SequenceExpr::FirstMatchSequenceExpr(expr) => {
                SequenceExpr::FirstMatch { expr: self.lower_sequence_expr(expr.expr()) }
            }
            ast::SequenceExpr::ParenthesizedSequenceExpr(expr) => SequenceExpr::Parenthesized {
                expr: self.lower_sequence_expr(expr.expr()),
                matches: expr
                    .match_list()
                    .map(|list| {
                        list.items().children().map(|item| self.lower_property_expr(item)).collect()
                    })
                    .unwrap_or_default(),
                repetition: expr
                    .repetition()
                    .map(|repetition| self.lower_sequence_repetition(repetition)),
            },
        };
        self.alloc_lowered_expr(Expr::Sequence(lowered), expr.syntax())
    }

    fn lower_sequence_repetition(
        &mut self,
        repetition: ast::SequenceRepetition,
    ) -> SequenceRepetition {
        SequenceRepetition {
            op: repetition.op().map(|token| token.kind()).unwrap_or(TokenKind::UNKNOWN),
            selector: repetition.selector().map(|selector| self.lower_selector(selector)),
        }
    }

    pub(crate) fn lower_property_expr(&mut self, expr: ast::PropertyExpr) -> ExprId {
        let lowered = match expr {
            ast::PropertyExpr::ParenthesizedPropertyExpr(expr) => PropertyExpr::Parenthesized {
                expr: self.lower_property_expr(expr.expr()),
                matches: expr
                    .match_list()
                    .map(|list| {
                        list.items().children().map(|item| self.lower_property_expr(item)).collect()
                    })
                    .unwrap_or_default(),
            },
            ast::PropertyExpr::BinaryPropertyExpr(expr) => PropertyExpr::Binary {
                left: self.lower_property_expr(expr.left()),
                op: expr.op().map(|token| token.kind()).unwrap_or(TokenKind::UNKNOWN),
                right: self.lower_property_expr(expr.right()),
            },
            ast::PropertyExpr::ConditionalPropertyExpr(expr) => PropertyExpr::Conditional {
                condition: self.lower_expr(expr.condition()),
                expr: self.lower_property_expr(expr.expr()),
                else_expr: expr.else_clause().map(|clause| self.lower_property_expr(clause.expr())),
            },
            ast::PropertyExpr::UnarySelectPropertyExpr(expr) => PropertyExpr::UnarySelect {
                op: expr.op().map(|token| token.kind()).unwrap_or(TokenKind::UNKNOWN),
                selector: expr.selector().map(|selector| self.lower_selector(selector)),
                expr: self.lower_property_expr(expr.expr()),
            },
            ast::PropertyExpr::ClockingPropertyExpr(expr) => PropertyExpr::Clocking {
                event: self.lower_timing_control(expr.event()),
                expr: expr.expr().map(|expr| self.lower_property_expr(expr)),
            },
            ast::PropertyExpr::SimplePropertyExpr(expr) => {
                PropertyExpr::Simple(self.lower_sequence_expr(expr.expr()))
            }
            ast::PropertyExpr::UnaryPropertyExpr(expr) => PropertyExpr::Unary {
                op: expr.op().map(|token| token.kind()).unwrap_or(TokenKind::UNKNOWN),
                expr: self.lower_property_expr(expr.expr()),
            },
            ast::PropertyExpr::StrongWeakPropertyExpr(expr) => PropertyExpr::StrongWeak {
                strong: expr
                    .keyword()
                    .map(|token| token.kind() == TokenKind::STRONG_KEYWORD)
                    .unwrap_or(false),
                expr: self.lower_sequence_expr(expr.expr()),
            },
            ast::PropertyExpr::AcceptOnPropertyExpr(expr) => PropertyExpr::AcceptOn {
                condition: self.lower_expr(expr.condition()),
                expr: self.lower_property_expr(expr.expr()),
            },
            ast::PropertyExpr::CasePropertyExpr(expr) => PropertyExpr::Case {
                expr: self.lower_expr(expr.expr()),
                items: expr
                    .items()
                    .children()
                    .map(|item| self.lower_property_case_item(item))
                    .collect(),
            },
        };
        self.alloc_lowered_expr(Expr::Property(lowered), expr.syntax())
    }

    fn lower_property_case_item(&mut self, item: ast::PropertyCaseItem) -> PropertyCaseItem {
        use ast::PropertyCaseItem::*;
        match item {
            DefaultPropertyCaseItem(item) => {
                PropertyCaseItem::Default { expr: self.lower_property_expr(item.expr()) }
            }
            StandardPropertyCaseItem(item) => PropertyCaseItem::Standard {
                expressions: item
                    .expressions()
                    .children()
                    .map(|expr| self.lower_expr(expr))
                    .collect(),
                expr: self.lower_property_expr(item.expr()),
            },
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
            BadExpression(bad) => Some(Expr::Error(bad.syntax().kind())),
            InsideExpression(expr) => self.lower_inside_expr(expr),
            TimingControlExpression(expr) => self.lower_timing_control_expr(expr),
            ExpressionOrDist(expr) => self.lower_expression_or_dist(expr),
            unsupported @ (ValueRangeExpression(_)
            | DataType(_)
            | TaggedUnionExpression(_)
            | NewArrayExpression(_)
            | NewClassExpression(_)
            | CopyClassExpression(_)
            | SuperNewDefaultedArgsExpression(_)
            | ArrayOrRandomizeMethodExpression(_)) => {
                Some(Expr::Unsupported(unsupported.syntax().kind()))
            }
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
            AssignmentPatternExpression(expr) => self.lower_assignment_pattern_expr(expr),
            EmptyQueueExpression(empty) => Some(Expr::Unsupported(empty.syntax().kind())),
        }
    }

    fn lower_assignment_pattern_expr(
        &mut self,
        expr: ast::AssignmentPatternExpression,
    ) -> Option<Expr> {
        let ty = expr.type_().map(|ty| self.lower_data_ty(ty));
        let pattern = self.lower_assignment_pattern(expr.pattern());
        Some(Expr::AssignmentPattern { ty, pattern })
    }

    fn lower_assignment_pattern(&mut self, pattern: ast::AssignmentPattern) -> AssignmentPattern {
        match pattern {
            ast::AssignmentPattern::SimpleAssignmentPattern(pattern) => AssignmentPattern::Simple(
                pattern.items().children().map(|expr| self.lower_expr(expr)).collect(),
            ),
            ast::AssignmentPattern::StructuredAssignmentPattern(pattern) => {
                AssignmentPattern::Structured(
                    pattern
                        .items()
                        .children()
                        .map(|item| {
                            let value = self.lower_expr(item.expr());
                            if item.key().syntax().kind()
                                == SyntaxKind::DEFAULT_PATTERN_KEY_EXPRESSION
                            {
                                AssignmentPatternItem::Default { value }
                            } else {
                                AssignmentPatternItem::KeyValue {
                                    key: self.lower_expr(item.key()),
                                    value,
                                }
                            }
                        })
                        .collect(),
                )
            }
            ast::AssignmentPattern::ReplicatedAssignmentPattern(pattern) => {
                AssignmentPattern::Replicated {
                    count: self.lower_expr(pattern.count_expr()),
                    items: pattern.items().children().map(|expr| self.lower_expr(expr)).collect(),
                }
            }
        }
    }

    fn lower_inside_expr(&mut self, expr: ast::InsideExpression) -> Option<Expr> {
        let value = self.lower_expr(expr.expr());
        let ranges = expr
            .ranges()
            .value_ranges()
            .children()
            .map(|range| {
                if let Some(range) = ast::ValueRangeExpression::cast(range.syntax()) {
                    InsideRange::Range {
                        left: self.lower_expr(range.left()),
                        right: self.lower_expr(range.right()),
                    }
                } else {
                    InsideRange::Expr(self.lower_expr(range))
                }
            })
            .collect();
        Some(Expr::Inside { expr: value, ranges })
    }

    fn lower_timing_control_expr(&mut self, expr: ast::TimingControlExpression) -> Option<Expr> {
        Some(Expr::TimingControl {
            control: self.lower_timing_control(expr.timing()),
            expr: self.lower_expr(expr.expr()),
        })
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
            Some(_) => return None,
        };
        let slice = expr.slice_size().map(|size| self.lower_expr(size));

        let concats = expr
            .expressions()
            .children()
            .map(|stream| StreamExpr {
                expr: self.lower_expr(stream.expression()),
                with_range: stream.with_range().map(|with_range| StreamRange {
                    selector: with_range
                        .range()
                        .selector()
                        .map(|selector| self.lower_selector(selector)),
                }),
            })
            .collect();
        Some(Expr::Stream { op, slice, concats })
    }

    fn lower_name(&mut self, name: ast::Name) -> Option<Expr> {
        fn lower_ident_select(
            ctx: &mut LoweringCtx<impl LoweringStore>,
            ident_select: ast::IdentifierSelectName,
            base: Option<ExprId>,
        ) -> Option<Expr> {
            let mut expr = match base {
                // `scope::member[sel]` keeps the scope receiver and applies
                // the selectors to the member field.
                Some(receiver) => {
                    Expr::Field { receiver, field: lower_ident_opt(ident_select.identifier()) }
                }
                None => {
                    lower_ident_opt(ident_select.identifier()).map_or(Expr::Missing, Expr::Ident)
                }
            };

            let mut selectors = ident_select
                .selectors()
                .children()
                .filter_map(|sel| Some(ctx.lower_selector(sel.selector()?)))
                .collect_vec()
                .into_iter()
                .peekable();

            let Some(expr_node) = ast::Expression::cast(ident_select.syntax()) else {
                return Some(expr);
            };
            let source = ctx.source_id(expr_node.syntax());
            loop {
                match selectors.next() {
                    select @ Some(_) => {
                        let (expressions, sources) = ctx.expressions();
                        let receiver = alloc_with_source_entry(expressions, sources, expr, source);
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
            ast::Name::IdentifierSelectName(ident_select) => {
                lower_ident_select(self, ident_select, None)
            }
            ast::Name::IdentifierName(ident) => {
                Some(lower_ident_opt(ident.identifier()).map_or(Expr::Missing, Expr::Ident))
            }
            ast::Name::ScopedName(scoped) => {
                let receiver = ast::Expression::cast(scoped.left().syntax())
                    .map(|left| self.lower_expr(left))
                    .unwrap_or_else(|| self.alloc_missing_expr());

                match scoped.right() {
                    IdentifierName(ident) => {
                        let field = lower_ident_opt(ident.identifier());
                        Some(Expr::Field { receiver, field })
                    }
                    IdentifierSelectName(ident_select) => {
                        lower_ident_select(self, ident_select, Some(receiver))
                    }
                    _ => Some(Expr::Missing),
                }
            }
            ast::Name::KeywordName(keyword) => {
                let ident = lower_ident(keyword.keyword());
                Some(ident.map_or(Expr::Missing, Expr::Ident))
            }
            ast::Name::ClassName(class_name) => {
                let ident = lower_ident_opt(class_name.identifier());
                Some(ident.map_or(Expr::Missing, Expr::Ident))
            }
            ast::Name::EmptyIdentifierName(_) => Some(Expr::Missing),
        }
    }

    fn lower_binary_expr(&mut self, expr: ast::BinaryExpression) -> Option<Expr> {
        let left = self.lower_expr(expr.left());
        let op = match expr.operator_token()?.kind() {
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

        let expr = ast::Expression::cast(expr.right().syntax())
            .map(|right| self.lower_expr(right))
            .unwrap_or_else(|| self.alloc_missing_expr());
        Some(Expr::Cast { ty, expr })
    }

    fn lower_cast_signed_expr(&mut self, expr: ast::SignedCastExpression) -> Option<Expr> {
        let signed = match expr.signing()?.kind() {
            TokenKind::SIGNED_KEYWORD => true,
            TokenKind::UNSIGNED_KEYWORD => false,
            _ => return None,
        };

        let expr = ast::Expression::cast(expr.inner().syntax())
            .map(|inner| self.lower_expr(inner))
            .unwrap_or_else(|| self.alloc_missing_expr());
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
                    None => self.alloc_missing_expr(),
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

    fn alloc_missing_expr(&mut self) -> ExprId {
        self.expressions().0.alloc(Expr::Missing)
    }
}
