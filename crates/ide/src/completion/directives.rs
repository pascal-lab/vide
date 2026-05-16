use std::sync::OnceLock;

use syntax::{SyntaxKind, SyntaxToken};

fn directive_kinds() -> &'static [SyntaxKind] {
    static KINDS: OnceLock<Vec<SyntaxKind>> = OnceLock::new();
    KINDS
        .get_or_init(|| {
            SyntaxKind::ALL
                .iter()
                .copied()
                .filter(|kind| !SyntaxToken::directive_text(*kind).is_empty())
                .collect()
        })
        .as_slice()
}

pub(super) fn is_directive_kind(kind: SyntaxKind) -> bool {
    directive_kinds().contains(&kind)
}

pub(super) fn directive_keywords() -> &'static Vec<String> {
    static DIRECTIVES: OnceLock<Vec<String>> = OnceLock::new();
    DIRECTIVES.get_or_init(|| {
        let mut items: Vec<String> = directive_kinds()
            .iter()
            .filter_map(|kind| {
                let text = syntax::SyntaxToken::directive_text(*kind);
                let text = text.trim_start_matches('`');
                (!text.is_empty()).then_some(text.to_string())
            })
            .collect();
        items.sort();
        items.dedup();
        items
    })
}
