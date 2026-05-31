use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId = CodeActionId {
    name: "expand_compound_assignment",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Expand compound assignment";
const COLLAPSE_ID: CodeActionId = CodeActionId {
    name: "collapse_compound_assignment",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const COLLAPSE_LABEL: &str = "Collapse compound assignment";

pub(super) fn expand_compound_assignment(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    expand_compound(collector, ctx).or_else(|| collapse_compound(collector, ctx))
}

fn expand_compound(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    let op_token = expr.operator_token()?;
    let op_text = op_token.value_text().to_string();
    let op = compound_operator(&op_text)?;

    let range = expr.syntax().text_range()?;
    let left_range = expr.left().syntax().text_range()?;
    let right_range = expr.right().syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let left = text.get(Range::from(left_range))?;
    let right = text.get(Range::from(right_range))?;

    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, format!("{} = {} {op} {}", left.trim(), left.trim(), right.trim()));
    });

    Some(())
}

fn collapse_compound(collector: &mut CodeActionCollector, ctx: &CodeActionCtx) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    if expr.operator_token()?.value_text().to_string() != "=" {
        return None;
    }

    let right = expr.right().as_binary_expression()?;
    let op_text = right.operator_token()?.value_text().to_string();
    let compound_op = simple_operator(&op_text)?;

    let text = ctx.sema().db.file_text(ctx.file_id());
    let left = text.get(Range::from(expr.left().syntax().text_range()?))?;
    let right_left = text.get(Range::from(right.left().syntax().text_range()?))?;
    let right_right = text.get(Range::from(right.right().syntax().text_range()?))?;
    if left.trim() != right_left.trim() {
        return None;
    }

    let range = expr.syntax().text_range()?;
    collector.add(COLLAPSE_ID, COLLAPSE_LABEL, range, |builder| {
        builder.replace(range, format!("{} {compound_op} {}", left.trim(), right_right.trim()));
    });

    Some(())
}

fn compound_operator(op: &str) -> Option<&'static str> {
    match op {
        "+=" => Some("+"),
        "-=" => Some("-"),
        "*=" => Some("*"),
        "/=" => Some("/"),
        "%=" => Some("%"),
        "&=" => Some("&"),
        "|=" => Some("|"),
        "^=" => Some("^"),
        "<<=" => Some("<<"),
        ">>=" => Some(">>"),
        "<<<=" => Some("<<<"),
        ">>>=" => Some(">>>"),
        _ => None,
    }
}

fn simple_operator(op: &str) -> Option<&'static str> {
    match op {
        "+" => Some("+="),
        "-" => Some("-="),
        "*" => Some("*="),
        "/" => Some("/="),
        "%" => Some("%="),
        "&" => Some("&="),
        "|" => Some("|="),
        "^" => Some("^="),
        "<<" => Some("<<="),
        ">>" => Some(">>="),
        "<<<" => Some("<<<="),
        ">>>" => Some(">>>="),
        _ => None,
    }
}
