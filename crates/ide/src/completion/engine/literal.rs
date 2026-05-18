use syntax::token::integer_literal_base_specifier_candidates;

use super::candidate::CompletionCandidate;
use crate::completion::context::CompletionContext;

pub(super) fn complete_integer_literal_bases(ctx: &CompletionContext) -> Vec<CompletionCandidate> {
    integer_literal_base_specifier_candidates()
        .into_iter()
        .map(|label| CompletionCandidate::text(label, ctx.replacement))
        .collect()
}
