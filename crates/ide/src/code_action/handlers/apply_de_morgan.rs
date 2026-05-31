use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use syntax::{
    SyntaxKind,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId =
    CodeActionId { name: "apply_de_morgan", kind: CodeActionKind::RefactorRewrite, repair: None };
const LABEL: &str = "Apply De Morgan's law";
const FACTOR_ID: CodeActionId =
    CodeActionId { name: "factor_de_morgan", kind: CodeActionKind::RefactorRewrite, repair: None };
const FACTOR_LABEL: &str = "Factor De Morgan's law";

pub(super) fn apply_de_morgan(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    if if_condition_de_morgan(collector, ctx).is_some() {
        return Some(());
    }

    let pushed = push_de_morgan(collector, ctx).is_some();
    let factored = factor_de_morgan(collector, ctx).is_some();
    pushed.then_some(()).or_else(|| factored.then_some(()))
}

fn push_de_morgan(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::PrefixUnaryExpression>()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let (range, replacement) = push_replacement(&text, expr)?;

    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}

fn factor_de_morgan(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    let op_range = expr.operator_token()?.text_range_in(expr.syntax())?;
    if !op_range.contains_range(ctx.range()) {
        return None;
    }

    let expr = same_op_root(expr);
    let text = ctx.sema().db.file_text(ctx.file_id());
    let (range, replacement) = factor_replacement(&text, expr)?;

    collector.add(FACTOR_ID, FACTOR_LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}

fn if_condition_de_morgan(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let if_stmt = ctx.find_node_at_offset::<ast::ConditionalStatement>()?;
    let predicate = if_stmt.predicate();
    let predicate_range = predicate.syntax().text_range()?;
    if !predicate_range.contains_range(ctx.range()) {
        return None;
    }

    let mut conditions = predicate.conditions().children();
    let condition = conditions.next()?;
    if conditions.next().is_some() || condition.matches_clause().is_some() {
        return None;
    }

    let text = ctx.sema().db.file_text(ctx.file_id());
    let expr = condition.expr();
    if let Some(prefix) = expr.as_prefix_unary_expression()
        && let Some((range, replacement)) = push_replacement(&text, prefix)
    {
        return collector.add(ID, LABEL, range, |builder| {
            builder.replace(range, replacement);
        });
    }

    let binary = condition_binary_expression(expr)?;
    let (range, replacement) = factor_replacement(&text, same_op_root(binary))?;
    collector.add(FACTOR_ID, FACTOR_LABEL, range, |builder| {
        builder.replace(range, replacement);
    })
}

fn push_replacement(
    text: &str,
    expr: ast::PrefixUnaryExpression<'_>,
) -> Option<(utils::text_edit::TextRange, String)> {
    if expr.as_unary_logical_not_expression().is_none() {
        return None;
    }

    let paren = expr.operand().as_primary_expression()?.as_parenthesized_expression()?;
    let inner = same_op_root(paren.expression().as_binary_expression()?);
    let op = logical_op(inner)?;

    let range = expr.syntax().text_range()?;
    let replacement = render_demorgan_terms(text, inner, op)?;
    Some((range, replacement))
}

fn factor_replacement(
    text: &str,
    expr: ast::BinaryExpression<'_>,
) -> Option<(utils::text_edit::TextRange, String)> {
    let op = logical_op(expr)?;
    let range = expr.syntax().text_range()?;
    let inner = render_demorgan_terms(text, expr, op)?;
    let (range, replacement) = match parenthesized_parent(expr) {
        Some(paren) => (paren.syntax().text_range()?, format!("!({inner})")),
        None => (range, format!("!({inner})")),
    };
    Some((range, replacement))
}

fn condition_binary_expression(expr: ast::Expression<'_>) -> Option<ast::BinaryExpression<'_>> {
    if let Some(binary) = expr.as_binary_expression() {
        return Some(binary);
    }

    expr.as_primary_expression()?.as_parenthesized_expression()?.expression().as_binary_expression()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogicalOp {
    And,
    Or,
}

impl LogicalOp {
    fn inverted(self) -> &'static str {
        match self {
            LogicalOp::And => "||",
            LogicalOp::Or => "&&",
        }
    }
}

fn logical_op(expr: ast::BinaryExpression<'_>) -> Option<LogicalOp> {
    match expr.operator_token()?.value_text().to_string().as_str() {
        "&&" => Some(LogicalOp::And),
        "||" => Some(LogicalOp::Or),
        _ => None,
    }
}

fn same_op_root<'a>(mut expr: ast::BinaryExpression<'a>) -> ast::BinaryExpression<'a> {
    let Some(op) = logical_op(expr) else {
        return expr;
    };

    while let Some(parent) = expr.syntax().parent().and_then(ast::BinaryExpression::cast)
        && logical_op(parent) == Some(op)
    {
        expr = parent;
    }

    expr
}

fn parenthesized_parent<'a>(
    expr: ast::BinaryExpression<'a>,
) -> Option<ast::ParenthesizedExpression<'a>> {
    expr.syntax().parent().and_then(ast::ParenthesizedExpression::cast)
}

fn render_demorgan_terms(
    text: &str,
    expr: ast::BinaryExpression<'_>,
    op: LogicalOp,
) -> Option<String> {
    let mut terms = Vec::new();
    collect_same_op_terms(expr, op, &mut terms);
    let inverted = terms
        .into_iter()
        .map(|expr| inverted_expression(text, expr))
        .collect::<Option<Vec<_>>>()?;
    Some(inverted.join(&format!(" {} ", op.inverted())))
}

fn collect_same_op_terms<'a>(
    expr: ast::BinaryExpression<'a>,
    op: LogicalOp,
    terms: &mut Vec<ast::Expression<'a>>,
) {
    for term in [expr.left(), expr.right()] {
        if let Some(binary) = term.as_binary_expression()
            && logical_op(binary) == Some(op)
        {
            collect_same_op_terms(binary, op, terms);
            continue;
        }

        terms.push(term);
    }
}

fn inverted_expression(text: &str, expr: ast::Expression<'_>) -> Option<String> {
    if let Some(operand) = negated_operand(expr) {
        return trimmed_text(text, operand);
    }

    if let Some(comparison) = expr.as_binary_expression()
        && let Some(inverted_op) = inverted_comparison_operator(comparison)
    {
        return replace_operator(text, comparison, inverted_op);
    }

    let text = trimmed_text(text, expr)?;
    Some(prefix_negation(expr, &text))
}

fn prefix_negation(expr: ast::Expression<'_>, text: &str) -> String {
    if expr.as_binary_expression().is_some() || expr.as_conditional_expression().is_some() {
        format!("!({text})")
    } else {
        format!("!{text}")
    }
}

fn negated_operand(expr: ast::Expression<'_>) -> Option<ast::Expression<'_>> {
    let unary = expr.as_prefix_unary_expression()?;
    unary.as_unary_logical_not_expression()?;
    Some(unary.operand())
}

fn inverted_comparison_operator(expr: ast::BinaryExpression<'_>) -> Option<&'static str> {
    match expr.syntax().kind() {
        SyntaxKind::EQUALITY_EXPRESSION => Some("!="),
        SyntaxKind::INEQUALITY_EXPRESSION => Some("=="),
        SyntaxKind::CASE_EQUALITY_EXPRESSION => Some("!=="),
        SyntaxKind::CASE_INEQUALITY_EXPRESSION => Some("==="),
        SyntaxKind::WILDCARD_EQUALITY_EXPRESSION => Some("!=?"),
        SyntaxKind::WILDCARD_INEQUALITY_EXPRESSION => Some("==?"),
        SyntaxKind::LESS_THAN_EXPRESSION => Some(">="),
        SyntaxKind::LESS_THAN_EQUAL_EXPRESSION => Some(">"),
        SyntaxKind::GREATER_THAN_EXPRESSION => Some("<="),
        SyntaxKind::GREATER_THAN_EQUAL_EXPRESSION => Some("<"),
        _ => None,
    }
}

fn replace_operator(
    text: &str,
    expr: ast::BinaryExpression<'_>,
    replacement: &str,
) -> Option<String> {
    let expr_range = expr.syntax().text_range()?;
    let op_range = expr.operator_token()?.text_range_in(expr.syntax())?;
    let expr_text = text.get(Range::from(expr_range))?;
    let op_start = usize::from(op_range.start() - expr_range.start());
    let op_end = usize::from(op_range.end() - expr_range.start());
    let mut result = String::new();
    result.push_str(expr_text.get(..op_start)?.trim_start());
    result.push_str(replacement);
    result.push_str(expr_text.get(op_end..)?.trim_end());
    Some(result)
}

fn trimmed_text(text: &str, expr: ast::Expression<'_>) -> Option<String> {
    let range = expr.syntax().text_range()?;
    Some(text.get(Range::from(range))?.trim().to_owned())
}
