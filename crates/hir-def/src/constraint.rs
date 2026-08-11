use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
};

use crate::{
    Ident,
    expr::{Expr, ExprId, InsideRange},
    lower::{LoweringCtx, LoweringStore},
    lower_ident_opt,
};

fn lower_constraint_name(name: ast::Name<'_>) -> Option<Ident> {
    name.as_identifier_name().and_then(|name| lower_ident_opt(name.identifier()))
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DistWeight {
    pub op: TokenKind,
    pub extra_op: Option<TokenKind>,
    pub expr: ExprId,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum DistItem {
    Range { range: InsideRange, weight: Option<DistWeight> },
    Default { weight: Option<DistWeight> },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct DistConstraintList {
    pub items: Box<[DistItem]>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Constraint {
    Expression {
        soft: bool,
        expr: ExprId,
    },
    Uniqueness {
        ranges: Box<[InsideRange]>,
    },
    Implication {
        left: ExprId,
        constraint: Idx<Constraint>,
    },
    Conditional {
        condition: ExprId,
        constraint: Idx<Constraint>,
        else_constraint: Option<Idx<Constraint>>,
    },
    Loop {
        array: ExprId,
        variables: SmallVec<[ExprId; 2]>,
        constraint: Idx<Constraint>,
    },
    Disable {
        soft: bool,
        name: ExprId,
    },
    SolveBefore {
        before: Box<[ExprId]>,
        after: Box<[ExprId]>,
    },
    Block(Box<[Idx<Constraint>]>),
    Unsupported(SyntaxKind),
}

pub type ConstraintId = Idx<Constraint>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ConstraintDef {
    pub name: Option<Ident>,
    pub constraint: ConstraintId,
    pub prototype: bool,
}

pub type ConstraintDefId = Idx<ConstraintDef>;

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn lower_expression_or_dist(&mut self, expr: ast::ExpressionOrDist) -> Option<Expr> {
        let value = self.lower_expr(expr.expr());
        let distribution = self.lower_dist_constraint_list(expr.distribution());
        Some(Expr::Dist { expr: value, distribution })
    }

    fn lower_dist_constraint_list(
        &mut self,
        distribution: ast::DistConstraintList,
    ) -> DistConstraintList {
        let items = distribution
            .items()
            .children()
            .map(|item| match item {
                ast::DistItemBase::DistItem(item) => DistItem::Range {
                    range: self.lower_inside_range(item.range()),
                    weight: item.weight().map(|weight| self.lower_dist_weight(weight)),
                },
                ast::DistItemBase::DefaultDistItem(item) => DistItem::Default {
                    weight: item.weight().map(|weight| self.lower_dist_weight(weight)),
                },
            })
            .collect();
        DistConstraintList { items }
    }

    fn lower_dist_weight(&mut self, weight: ast::DistWeight) -> DistWeight {
        DistWeight {
            op: weight.op().map(|token| token.kind()).unwrap_or(TokenKind::UNKNOWN),
            extra_op: weight.extra_op().map(|token| token.kind()),
            expr: self.lower_expr(weight.expr()),
        }
    }

    fn lower_inside_range(&mut self, range: ast::Expression) -> InsideRange {
        if let ast::Expression::PrimaryExpression(
            ast::PrimaryExpression::ParenthesizedExpression(expr),
        ) = &range
        {
            return InsideRange::Expr(self.lower_expr(expr.expression()));
        }
        if let Some(range) = ast::ValueRangeExpression::cast(range.syntax()) {
            return InsideRange::Range {
                left: self.lower_expr(range.left()),
                right: self.lower_expr(range.right()),
            };
        }
        InsideRange::Expr(self.lower_expr(range))
    }

    pub(crate) fn lower_constraint_decl(
        &mut self,
        declaration: ast::ConstraintDeclaration,
    ) -> ConstraintDefId {
        let constraint =
            self.lower_constraint_item(ast::ConstraintItem::ConstraintBlock(declaration.block()));
        let source = self.source_id(declaration.syntax());
        let (body, sources) = self.store.body();
        crate::alloc_with_source_entry(
            &mut body.constraint_defs,
            &mut sources.constraint_def_srcs,
            ConstraintDef {
                name: lower_constraint_name(declaration.name()),
                constraint,
                prototype: false,
            },
            source,
        )
    }

    pub(crate) fn lower_constraint_prototype(
        &mut self,
        prototype: ast::ConstraintPrototype,
    ) -> ConstraintDefId {
        let source = self.source_id(prototype.syntax());
        let constraint = self.alloc_constraint(Constraint::Block(Box::new([])), prototype.syntax());
        let (body, sources) = self.store.body();
        crate::alloc_with_source_entry(
            &mut body.constraint_defs,
            &mut sources.constraint_def_srcs,
            ConstraintDef {
                name: lower_constraint_name(prototype.name()),
                constraint,
                prototype: true,
            },
            source,
        )
    }

    pub(crate) fn lower_inline_constraint_block(
        &mut self,
        block: ast::ConstraintBlock,
    ) -> ConstraintId {
        self.lower_constraint_item(ast::ConstraintItem::ConstraintBlock(block))
    }

    fn lower_constraint_item(&mut self, item: ast::ConstraintItem) -> ConstraintId {
        let constraint = match item {
            ast::ConstraintItem::ExpressionConstraint(item) => Constraint::Expression {
                soft: item.soft().is_some(),
                expr: self.lower_expr(item.expr()),
            },
            ast::ConstraintItem::UniquenessConstraint(item) => Constraint::Uniqueness {
                ranges: item
                    .ranges()
                    .value_ranges()
                    .children()
                    .map(|range| self.lower_inside_range(range))
                    .collect(),
            },
            ast::ConstraintItem::ImplicationConstraint(item) => Constraint::Implication {
                left: self.lower_expr(item.left()),
                constraint: self.lower_constraint_item(item.constraints()),
            },
            ast::ConstraintItem::ConditionalConstraint(item) => Constraint::Conditional {
                condition: self.lower_expr(item.condition()),
                constraint: self.lower_constraint_item(item.constraints()),
                else_constraint: item
                    .else_clause()
                    .map(|clause| self.lower_constraint_item(clause.constraints())),
            },
            ast::ConstraintItem::LoopConstraint(item) => {
                let list = item.loop_list();
                Constraint::Loop {
                    array: self.lower_expr(list.array_name()),
                    variables: list
                        .loop_variables()
                        .children()
                        .filter_map(|name| ast::Expression::cast(name.syntax()))
                        .map(|name| self.lower_expr(name))
                        .collect(),
                    constraint: self.lower_constraint_item(item.constraints()),
                }
            }
            ast::ConstraintItem::DisableConstraint(item) => Constraint::Disable {
                soft: item.soft().is_some(),
                name: self.lower_expr(item.name()),
            },
            ast::ConstraintItem::SolveBeforeConstraint(item) => Constraint::SolveBefore {
                before: item.before_expr().children().map(|expr| self.lower_expr(expr)).collect(),
                after: item.after_expr().children().map(|expr| self.lower_expr(expr)).collect(),
            },
            ast::ConstraintItem::ConstraintBlock(item) => Constraint::Block(
                item.items().children().map(|item| self.lower_constraint_item(item)).collect(),
            ),
        };
        self.alloc_constraint(constraint, item.syntax())
    }

    fn alloc_constraint(
        &mut self,
        constraint: Constraint,
        syntax: syntax::SyntaxNode<'_>,
    ) -> ConstraintId {
        let source = self.source_id(syntax);
        let (body, sources) = self.store.body();
        crate::alloc_with_source_entry(
            &mut body.constraints,
            &mut sources.constraint_srcs,
            constraint,
            source,
        )
    }
}
