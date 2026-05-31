use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent, newline_style,
    text_at,
};

const ID: CodeActionId = CodeActionId {
    name: "wrap_statement_in_begin_end",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Wrap statement in begin/end";

pub(super) fn wrap_statement_in_begin_end(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let stmt = ctx.find_node_at_offset::<ast::Statement>()?;
    if stmt.as_block_statement().is_some() {
        return None;
    }

    let range = stmt.syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let stmt_text = text_at(&text, range)?;
    let newline = newline_style(&text);
    let indent = line_indent(&text, range.start());
    let replacement = format!("begin{newline}{indent}    {}{newline}{indent}end", stmt_text.trim());

    collector.add(ID, LABEL, range, |builder| {
        builder.replace(range, replacement);
    });

    Some(())
}
