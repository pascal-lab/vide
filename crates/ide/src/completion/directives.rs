use std::sync::OnceLock;

use syntax::SyntaxKind;

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

pub(super) fn is_directive_kind(kind: SyntaxKind) -> bool {
    DIRECTIVE_KINDS.contains(&kind)
}

pub(super) fn directive_keywords() -> &'static Vec<String> {
    static DIRECTIVES: OnceLock<Vec<String>> = OnceLock::new();
    DIRECTIVES.get_or_init(|| {
        let mut items: Vec<String> = DIRECTIVE_KINDS
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
