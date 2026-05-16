use std::collections::HashMap;

use super::candidate::CompletionCandidate;
use crate::completion::{context::CompletionContext, directives, engine::snippets};

pub(super) fn complete_directives(ctx: &CompletionContext) -> Vec<CompletionCandidate> {
    let snippet_entries = snippets::entries(&snippets::snippet_config().directives);
    let mut snippet_map = HashMap::new();
    for entry in snippet_entries {
        snippet_map.insert(entry.label.clone(), entry);
    }

    let mut items = Vec::new();
    for kw in directives::directive_keywords().iter().filter(|kw| kw.starts_with(&ctx.prefix)) {
        if let Some(entry) = snippet_map.get(kw) {
            items.push(CompletionCandidate::snippet(
                entry.label.clone(),
                ctx.replacement,
                entry.plain.clone(),
                entry.snippet.clone(),
            ));
        }
        items.push(CompletionCandidate::keyword(kw.clone(), ctx.replacement));
    }

    items
}
