use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, text_at,
};

const ID: CodeActionId =
    CodeActionId { name: "invert_if_else", kind: CodeActionKind::RefactorRewrite, repair: None };
const LABEL: &str = "Invert if/else";

pub(super) fn invert_if_else(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let if_stmt = ctx.find_node_at_offset::<ast::ConditionalStatement>()?;
    let else_clause = if_stmt.else_clause()?;

    let pred_range = if_stmt.predicate().syntax().text_range()?;
    let then_range = if_stmt.statement().syntax().text_range()?;
    let else_range = else_clause.clause().syntax().text_range()?;

    let text = ctx.sema().db.file_text(ctx.file_id());
    let predicate = text_at(&text, pred_range)?;
    let then_text = text_at(&text, then_range)?;
    let else_text = text_at(&text, else_range)?;

    collector.add(ID, LABEL, if_stmt.syntax().text_range()?, |builder| {
        builder.replace(pred_range, format!("!({})", predicate.trim()));
        builder.replace(then_range, else_text.trim().to_owned());
        builder.replace(else_range, then_text.trim().to_owned());
    });

    Some(())
}
