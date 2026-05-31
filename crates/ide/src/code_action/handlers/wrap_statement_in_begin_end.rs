use std::ops::Range;

use hir::base_db::source_db::SourceDb;
use syntax::{
    SyntaxNode,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, line_indent,
};

const WRAP_ID: CodeActionId = CodeActionId {
    name: "wrap_statement_in_begin_end",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const WRAP_LABEL: &str = "Wrap statement in begin/end";

pub(super) fn wrap_statement_in_begin_end(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let stmt = ctx.find_node_at_offset::<ast::Statement>()?;
    if stmt.as_block_statement().is_some() || !is_control_flow_body(stmt.syntax()) {
        return None;
    }

    let range = stmt.syntax().text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let stmt_text = text.get(Range::from(range))?;

    collector.add(WRAP_ID, WRAP_LABEL, range, |builder| {
        let indent = line_indent(&text, range.start());
        let replacement = format!("begin\n{indent}    {}\n{indent}end", stmt_text.trim());
        builder.replace(range, replacement);
    });

    Some(())
}

const UNWRAP_ID: CodeActionId = CodeActionId {
    name: "unwrap_single_statement_block",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const UNWRAP_LABEL: &str = "Unwrap single-statement begin/end";

pub(super) fn unwrap_single_statement_block(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let block = ctx.find_node_at_offset::<ast::BlockStatement>()?;
    if !is_control_flow_body(block.syntax()) {
        return None;
    }

    let mut items = block.items().children();
    let item = items.next()?.syntax();
    if !ast::Statement::can_cast(item.kind()) || items.next().is_some() {
        return None;
    }

    let block_range = block.syntax().text_range()?;
    let item_range = item.text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let replacement = text.get(Range::from(item_range))?.trim();

    collector.add(UNWRAP_ID, UNWRAP_LABEL, block_range, |builder| {
        builder.replace(block_range, replacement);
    });

    Some(())
}

fn is_control_flow_body(syntax: SyntaxNode<'_>) -> bool {
    let Some(parent) = syntax.parent() else {
        return false;
    };

    ast::ConditionalStatement::cast(parent)
        .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::ElseClause::cast(parent).is_some_and(|parent| parent.clause().syntax() == syntax)
        || ast::ForLoopStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::ForeachLoopStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::ForeverStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::LoopStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::DoWhileStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::WaitStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::TimingControlStatement::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
        || ast::StandardCaseItem::cast(parent)
            .is_some_and(|parent| parent.clause().syntax() == syntax)
        || ast::DefaultCaseItem::cast(parent)
            .is_some_and(|parent| parent.clause().syntax() == syntax)
        || ast::PatternCaseItem::cast(parent)
            .is_some_and(|parent| parent.statement().syntax() == syntax)
}
