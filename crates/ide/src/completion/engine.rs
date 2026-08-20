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
    analysis::AnalysisContext,
    completion::{
        context::{CompletionContext, TriggerChar, completion_context},
        request::CompletionRequest,
    },
};

pub(crate) fn completions(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    trigger: Option<TriggerChar>,
) -> Vec<CompletionItem> {
    if db.file_kind(position.file_id).is_project_manifest() {
        return crate::manifest::completions(db.db, position);
    }
    let ctx = completion_context(db, position, trigger);
    completions_with_context(db, position, &ctx)
}

fn completions_with_context(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let Some(request) = CompletionRequest::from_context(ctx) else {
        return Vec::new();
    };

    plan::complete_request(db, position, ctx, request)
}
