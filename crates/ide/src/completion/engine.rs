mod expr;
mod instantiation;
mod item;
mod keywords;
mod member;
mod named;
mod paren_list;
mod plan;
mod port_list;
mod preproc;
mod sensitivity_list;
mod snippets;
mod typed_filter;

#[cfg(test)]
mod tests;

use ide_db::root_db::RootDb;
use span::FilePosition;

pub use self::item::{CompletionItem, CompletionItemKind};
use crate::completion::context::{CompletionContext, TriggerChar, completion_context};

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
    plan::CompletionPlan::from_context(ctx).complete(db, position, ctx)
}
