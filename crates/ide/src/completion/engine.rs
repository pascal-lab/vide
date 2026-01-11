mod named;
mod typed_filter;

#[cfg(test)]
mod tests;

use ide_db::root_db::RootDb;
use span::FilePosition;

pub use self::named::CompletionItem;
use crate::completion::context::{
    CompletionContext, DotKind, LexContext, Qualifier, TriggerChar, completion_context,
};

pub fn completions(
    db: &RootDb,
    position: FilePosition,
    trigger: Option<TriggerChar>,
) -> Vec<CompletionItem> {
    let ctx = completion_context(db, position, trigger);
    completions_with_context(db, position, &ctx)
}

fn completions_with_context(
    db: &RootDb,
    position: FilePosition,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    if ctx.lex != LexContext::Code {
        return Vec::new();
    }

    match ctx.qualifier {
        Some(Qualifier::AfterDot(after_dot)) => match after_dot.kind {
            DotKind::NamedPort => named::complete_named_port_names(db, position, &ctx.prefix, ctx),
            DotKind::NamedParam => {
                named::complete_named_param_names(db, position, &ctx.prefix, ctx)
            }
            DotKind::Member => Vec::new(),
        },
        Some(Qualifier::InNamedPortConnExpr) => {
            named::complete_named_port_conn_expr(db, position, &ctx.prefix, ctx)
        }
        Some(Qualifier::InNamedParamAssignExpr) => {
            named::complete_named_param_assign_expr(db, position, &ctx.prefix, ctx)
        }
        Some(Qualifier::AfterHash(_)) => Vec::new(),
        Some(Qualifier::InParenList(_)) => Vec::new(),
        Some(Qualifier::AfterAt(_)) => Vec::new(),
        Some(Qualifier::AfterBacktick) => Vec::new(),
        None => Vec::new(),
    }
}
