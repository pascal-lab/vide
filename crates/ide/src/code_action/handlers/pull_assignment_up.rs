use std::{borrow::Cow, ops::Range};

use hir::base_db::source_db::SourceDb;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId = CodeActionId {
    name: "pull_assignment_up",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const DOWN_ID: CodeActionId = CodeActionId {
    name: "pull_assignment_down",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};

pub(super) fn pull_assignment_up(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let mut conditional = ctx.find_node_at_offset::<ast::ConditionalStatement>()?;
    while let Some(parent_if) = conditional
        .syntax()
        .parent()
        .and_then(|node| ast::ElseClause::cast(node)?.syntax().parent())
        .and_then(ast::ConditionalStatement::cast)
    {
        conditional = parent_if;
    }

    let text = ctx.sema().db.file_text(ctx.file_id());
    let (lhs, expr) = conditional_assignment_expression(conditional, &text)?;

    collector.add(ID, "Pull assignment up", conditional.syntax().text_range()?, |builder| {
        let replacement = format!("{} = {};", lhs.trim(), expr);
        builder.replace(conditional.syntax().text_range().unwrap(), replacement);
    })
}

pub(super) fn pull_assignment_down(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let assignment = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    if assignment.operator_token()?.kind() != TokenKind::EQUALS {
        return None;
    }

    let conditional = assignment.right().as_conditional_expression()?;
    let stmt = syntax::SyntaxAncestors::start_from(assignment.syntax())
        .find_map(ast::ExpressionStatement::cast)?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let lhs = text.get(Range::from(assignment.left().syntax().text_range()?))?.trim();
    let replacement = conditional_assignment_statement(conditional, lhs, &text)?;

    collector.add(DOWN_ID, "Pull assignment down", stmt.syntax().text_range()?, |builder| {
        builder.replace(stmt.syntax().text_range().unwrap(), replacement);
    })
}

fn conditional_assignment_expression<'a>(
    conditional: ast::ConditionalStatement<'_>,
    text: &'a str,
) -> Option<(&'a str, String)> {
    let (lhs, then_rhs) = assignment_rhs_text(conditional.statement(), text)?;

    let else_syntax = conditional.else_clause()?.clause().syntax();
    let (else_lhs, else_expr) = if let Some(nested) = ast::ConditionalStatement::cast(else_syntax) {
        conditional_assignment_expression(nested, text)?
    } else {
        let else_stmt = ast::Statement::cast(else_syntax)?;
        let (lhs, expr) = assignment_rhs_text(else_stmt, text)?;
        (lhs, expr.to_owned())
    };

    if else_lhs != lhs {
        return None;
    }

    let predicate: Cow<'a, str> = {
        let predicate =
            text.get(Range::from(conditional.predicate().syntax().text_range()?))?.trim();

        if predicate.contains('?') { format!("({predicate})").into() } else { predicate.into() }
    };
    Some((lhs, format!("{predicate} ? {then_rhs} : {else_expr}")))
}

fn assignment_rhs_text<'a>(stmt: ast::Statement<'_>, text: &'a str) -> Option<(&'a str, &'a str)> {
    if let Some(block) = stmt.as_block_statement() {
        let item = block.items().only_children()?;
        let stmt = ast::Statement::cast(item.syntax())?;
        return assignment_rhs_text(stmt, text);
    }

    let assignment = stmt.as_expression_statement()?.expr().as_binary_expression()?;
    if assignment.operator_token()?.kind() != TokenKind::EQUALS {
        return None;
    }

    let lhs = text.get(Range::from(assignment.left().syntax().text_range()?))?.trim();
    let rhs = text.get(Range::from(assignment.right().syntax().text_range()?))?.trim();
    Some((lhs, rhs))
}

fn conditional_assignment_statement(
    conditional: ast::ConditionalExpression<'_>,
    lhs: &str,
    text: &str,
) -> Option<String> {
    let predicate = text.get(Range::from(conditional.predicate().syntax().text_range()?))?.trim();
    let then_expr = expr_text(conditional.left(), text)?;
    let else_expr = if let Some(nested) = conditional.right().as_conditional_expression() {
        conditional_assignment_statement(nested, lhs, text)?
    } else {
        format!("{lhs} = {};", expr_text(conditional.right(), text)?)
    };
    Some(format!("if ({predicate}) {lhs} = {then_expr}; else {else_expr}"))
}

fn expr_text<'a>(expr: ast::Expression<'_>, text: &'a str) -> Option<&'a str> {
    text.get(Range::from(expr.syntax().text_range()?)).map(str::trim)
}
