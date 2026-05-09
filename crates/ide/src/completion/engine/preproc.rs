use std::collections::HashMap;

use utils::text_edit::TextEditItem;

use super::{CompletionItem, CompletionItemKind};
use crate::completion::{context::CompletionContext, directives, engine::snippets};

pub(super) fn complete_directives(ctx: &CompletionContext) -> Vec<CompletionItem> {
    let snippet_entries = snippets::entries(&snippets::snippet_config().directives);
    let mut snippet_map = HashMap::new();
    for entry in snippet_entries {
        snippet_map.insert(entry.label.clone(), entry);
    }

    let mut items = Vec::new();
    for kw in directives::directive_keywords().iter().filter(|kw| kw.starts_with(&ctx.prefix)) {
        if let Some(entry) = snippet_map.get(kw) {
            items.push(CompletionItem {
                label: entry.label.clone(),
                kind: CompletionItemKind::Snippet,
                edit: Some(TextEditItem::replace(ctx.replacement, entry.plain.clone())),
                snippet_edit: Some(TextEditItem::replace(ctx.replacement, entry.snippet.clone())),
            });
        }
        items.push(CompletionItem {
            label: kw.clone(),
            kind: CompletionItemKind::Keyword,
            edit: Some(TextEditItem::replace(ctx.replacement, kw.clone())),
            snippet_edit: None,
        });
    }

    items
}
