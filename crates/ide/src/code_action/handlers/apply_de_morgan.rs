use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, text_at,
};

const ID: CodeActionId =
    CodeActionId { name: "apply_de_morgan", kind: CodeActionKind::RefactorRewrite, repair: None };
const LABEL: &str = "Apply De Morgan's law";

pub(super) fn apply_de_morgan(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::PrefixUnaryExpression>()?;
    if expr.as_unary_logical_not_expression().is_none() {
        return None;
    }

    let paren = expr.operand().as_primary_expression()?.as_parenthesized_expression()?;
    let inner = paren.expression().as_binary_expression()?;
    let op_text = inner.operator_token()?.value_text().to_string();
    let op = match op_text.as_str() {
        "&&" => "||",
        "||" => "&&",
        _ => return None,
    };

    let range = expr.syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let left = text_at(&text, inner.left().syntax().text_range()?)?;
    let right = text_at(&text, inner.right().syntax().text_range()?)?;
    let replacement = format!(
        "{} {op} {}",
        negated(inner.left(), left.trim()),
        negated(inner.right(), right.trim())
    );

    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}

fn negated(expr: ast::Expression<'_>, text: &str) -> String {
    if expr.as_binary_expression().is_some() || expr.as_conditional_expression().is_some() {
        format!("!({text})")
    } else {
        format!("!{text}")
    }
}
