use super::candidate::CompletionCandidate;
use crate::completion::context::CompletionContext;

pub(super) fn complete_integer_literal_bases(ctx: &CompletionContext) -> Vec<CompletionCandidate> {
    ["b", "sb", "o", "so", "d", "sd", "h", "sh"]
        .into_iter()
        .map(|label| CompletionCandidate::text(label.to_owned(), ctx.replacement))
        .collect()
}
