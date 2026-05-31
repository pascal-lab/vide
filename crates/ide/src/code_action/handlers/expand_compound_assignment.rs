use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, text_at,
};

const ID: CodeActionId = CodeActionId {
    name: "expand_compound_assignment",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Expand compound assignment";

pub(super) fn expand_compound_assignment(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::BinaryExpression>()?;
    let op_token = expr.operator_token()?;
    let op_text = op_token.value_text().to_string();
    let op = compound_operator(&op_text)?;

    let range = expr.syntax().text_range()?;
    let left_range = expr.left().syntax().text_range()?;
    let right_range = expr.right().syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let left = text_at(&text, left_range)?;
    let right = text_at(&text, right_range)?;
    let replacement = format!("{} = {} {op} {}", left.trim(), left.trim(), right.trim());

    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
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
