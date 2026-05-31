use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use crate::code_action::{
    CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind, text_at,
};

const ID: CodeActionId = CodeActionId {
    name: "unwrap_single_statement_block",
    kind: CodeActionKind::RefactorRewrite,
    repair: None,
};
const LABEL: &str = "Unwrap single-statement begin/end";

pub(super) fn unwrap_single_statement_block(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let block = ctx.find_node_at_offset::<ast::BlockStatement>()?;
    if !is_control_flow_body(block) {
        return None;
    }

    let mut items = block.items().children();
    let item = items.next()?;
    if items.next().is_some() {
        return None;
    }

    let item_syntax = item.syntax();
    ast::Statement::cast(item_syntax)?;
    let block_range = block.syntax().text_range()?;
    let item_range = item_syntax.text_range()?;
    let text = ctx.sema().db.file_text(ctx.file_id());
    let replacement = text_at(&text, item_range)?.trim().to_owned();

    collector.add(ID, LABEL, block_range, |builder| {
        builder.replace(block_range, replacement);
    });

    Some(())
}

fn is_control_flow_body(block: ast::BlockStatement<'_>) -> bool {
    let syntax = block.syntax();
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
