use std::{borrow::Cow, ops::Range};

use hir::base_db::source_db::SourceDb;
use syntax::{
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextRange;

use crate::code_action::{CodeActionCollector, CodeActionCtx, CodeActionId, CodeActionKind};

const ID: CodeActionId =
    CodeActionId { name: "merge_nested_if", kind: CodeActionKind::RefactorRewrite, repair: None };

// Assist: merge_nested_if
//
// This merges nested if statements without else branches into one if statement
// with a combined condition.
//
// ```
// always_comb if$0 (a) begin if (b) y = 1; end
// ```
// ->
// ```
// always_comb if (a && b) y = 1;
// ```
pub(super) fn merge_nested_if(
    collector: &mut CodeActionCollector,
    ctx: &CodeActionCtx,
) -> Option<()> {
    let current_if = ctx.find_node_at_offset::<ast::ConditionalStatement>()?;
    if !in_if_head(current_if, ctx.range()) || current_if.else_clause().is_some() {
        return None;
    }

    let outer_if = outermost_mergeable_if(current_if);
    let chain = nested_if_chain(outer_if);
    if chain.len() < 2 {
        return None;
    }

    let innermost_if = *chain.last()?;
    let innermost_body_stmt = single_statement_body(innermost_if.statement())?;

    let text = ctx.sema().db.file_text(ctx.file_id());
    let predicates = chain
        .iter()
        .map(|if_stmt| {
            let range = if_stmt.predicate().syntax().text_range()?;
            let predicate = text.get(Range::from(range))?.trim();
            if predicate.contains("||") || predicate.contains('?') {
                Some(Cow::Owned(format!("({predicate})")))
            } else {
                Some(Cow::Borrowed(predicate))
            }
        })
        .collect::<Option<Vec<_>>>()?;

    let outer_pred_range = outer_if.predicate().syntax().text_range()?;
    let outer_body_range = outer_if.statement().syntax().text_range()?;

    let innermost_body_range = innermost_body_stmt.syntax().text_range()?;
    let innermost_body = text.get(Range::from(innermost_body_range))?.trim().to_owned();

    collector.add(ID, "Merge nested if", outer_if.syntax().text_range()?, |builder| {
        let merged_predicate = predicates.join(" && ");
        builder.replace(outer_pred_range, merged_predicate);
        builder.replace(outer_body_range, innermost_body);
    })
}

fn in_if_head(if_stmt: ast::ConditionalStatement<'_>, range: TextRange) -> bool {
    let Some(if_range) = if_stmt.syntax().text_range() else { return false };
    let Some(pred_range) = if_stmt.predicate().syntax().text_range() else { return false };
    TextRange::new(if_range.start(), pred_range.end()).contains_range(range)
}

fn outermost_mergeable_if<'a>(
    mut if_stmt: ast::ConditionalStatement<'a>,
) -> ast::ConditionalStatement<'a> {
    while let Some(parent_if) = parent_conditional_statement(if_stmt) {
        if parent_if.else_clause().is_some() {
            break;
        }
        let Some(body) = single_statement_body(parent_if.statement()) else { break };
        let Some(body_stmt) = body.as_conditional_statement() else { break };
        if body_stmt.syntax() != if_stmt.syntax() {
            break;
        }
        if_stmt = parent_if;
    }

    if_stmt
}

fn parent_conditional_statement<'a>(
    if_stmt: ast::ConditionalStatement<'a>,
) -> Option<ast::ConditionalStatement<'a>> {
    let mut parent = if_stmt.syntax().parent();
    while let Some(node) = parent {
        if let Some(parent_if) = ast::ConditionalStatement::cast(node) {
            return Some(parent_if);
        }
        parent = node.parent();
    }
    None
}

fn nested_if_chain<'a>(
    outer_if: ast::ConditionalStatement<'a>,
) -> Vec<ast::ConditionalStatement<'a>> {
    let mut chain = vec![outer_if];
    let mut current_if = outer_if;
    while let Some(body) = single_statement_body(current_if.statement()) {
        let Some(nested_if) = body.as_conditional_statement() else {
            break;
        };
        if nested_if.else_clause().is_some() {
            break;
        }
        chain.push(nested_if);
        current_if = nested_if;
    }
    chain
}

fn single_statement_body(stmt: ast::Statement<'_>) -> Option<ast::Statement<'_>> {
    let Some(block) = stmt.as_block_statement() else {
        return Some(stmt);
    };

    let mut items = block.items().children();
    let item = items.next()?;
    if items.next().is_some() {
        return None;
    }
    ast::Statement::cast(item.syntax())
}
