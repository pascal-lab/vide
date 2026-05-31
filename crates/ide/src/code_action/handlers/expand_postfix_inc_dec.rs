use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, text_at,
};

const ID: CodeActionId = CodeActionId {
    name: "expand_postfix_inc_dec",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Expand postfix increment/decrement";

pub(super) fn expand_postfix_inc_dec(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let expr = ctx.find_node_at_offset::<ast::PostfixUnaryExpression>()?;
    let op = expr.operator_token()?.value_text().to_string();
    let binary_op = match op.as_str() {
        "++" => "+",
        "--" => "-",
        _ => return None,
    };

    let range = expr.syntax().text_range()?;
    let operand_range = expr.operand().syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let operand = text_at(&text, operand_range)?;
    let replacement = format!("{operand} = {operand} {binary_op} 1");

    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}
