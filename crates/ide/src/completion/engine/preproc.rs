use std::{collections::HashMap, sync::OnceLock};

use syntax::SyntaxKind;
use utils::text_edit::TextEditItem;

use super::{CompletionItem, CompletionItemKind};
use crate::completion::context::CompletionContext;
use crate::completion::engine::snippets;

const DIRECTIVE_KINDS: &[SyntaxKind] = &[
    SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE,
    SyntaxKind::CELL_DEFINE_DIRECTIVE,
    SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE,
    SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE,
    SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE,
    SyntaxKind::DEFINE_DIRECTIVE,
    SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE,
    SyntaxKind::DELAY_MODE_PATH_DIRECTIVE,
    SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE,
    SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE,
    SyntaxKind::ELSE_DIRECTIVE,
    SyntaxKind::ELS_IF_DIRECTIVE,
    SyntaxKind::END_CELL_DEFINE_DIRECTIVE,
    SyntaxKind::END_IF_DIRECTIVE,
    SyntaxKind::END_KEYWORDS_DIRECTIVE,
    SyntaxKind::END_PROTECT_DIRECTIVE,
    SyntaxKind::END_PROTECTED_DIRECTIVE,
    SyntaxKind::IF_DEF_DIRECTIVE,
    SyntaxKind::IF_N_DEF_DIRECTIVE,
    SyntaxKind::INCLUDE_DIRECTIVE,
    SyntaxKind::LINE_DIRECTIVE,
    SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE,
    SyntaxKind::PRAGMA_DIRECTIVE,
    SyntaxKind::PROTECT_DIRECTIVE,
    SyntaxKind::PROTECTED_DIRECTIVE,
    SyntaxKind::RESET_ALL_DIRECTIVE,
    SyntaxKind::TIME_SCALE_DIRECTIVE,
    SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE,
    SyntaxKind::UNDEF_DIRECTIVE,
    SyntaxKind::UNDEFINE_ALL_DIRECTIVE,
    SyntaxKind::BIND_DIRECTIVE,
];

pub(super) fn complete_directives(prefix: &str, ctx: &CompletionContext) -> Vec<CompletionItem> {
    let prefix = prefix.strip_prefix('`').unwrap_or(prefix);
    let snippet_entries = snippets::entries(&snippets::snippet_config().directives);
    let mut snippet_map = HashMap::new();
    for entry in snippet_entries {
        snippet_map.insert(entry.label.clone(), entry);
    }

    let mut items = Vec::new();
    for kw in directive_keywords().iter().filter(|kw| kw.starts_with(prefix)) {
        if let Some(entry) = snippet_map.get(kw) {
            items.push(CompletionItem {
                label: entry.label.clone(),
                kind: CompletionItemKind::Snippet,
                edit: Some(TextEditItem::replace(ctx.replacement, entry.plain.clone())),
                snippet_edit: Some(TextEditItem::replace(
                    ctx.replacement,
                    entry.snippet.clone(),
                )),
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

fn directive_keywords() -> &'static Vec<String> {
    static DIRECTIVES: OnceLock<Vec<String>> = OnceLock::new();
    DIRECTIVES.get_or_init(|| {
        let mut items: Vec<String> = DIRECTIVE_KINDS
            .iter()
            .filter_map(|kind| {
                let text = syntax::directive_text(*kind);
                let text = text.trim_start_matches('`');
                (!text.is_empty()).then_some(text.to_string())
            })
            .collect();
        items.sort();
        items.dedup();
        items
    })
}
