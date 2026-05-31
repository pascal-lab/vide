mod candidate;
mod expr;
mod instantiation;
mod item;
mod keywords;
mod literal;
mod member;
mod named;
mod paren_list;
mod plan;
mod port_list;
mod preproc;
mod sensitivity_list;
mod snippets;
mod system;
mod typed_filter;

#[cfg(test)]
mod tests;

pub use self::item::{CompletionItem, CompletionItemKind};
use crate::{
    FilePosition,
    completion::{
        context::{CompletionContext, TriggerChar, completion_context},
        request::CompletionRequest,
    },
    db::root_db::RootDb,
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
    let Some(request) = CompletionRequest::from_context(ctx) else {
        return Vec::new();
    };

    plan::complete_request(db, position, ctx, request)
}
