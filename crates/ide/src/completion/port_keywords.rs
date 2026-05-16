use std::sync::OnceLock;

use syntax::{SyntaxFacts, SyntaxToken, TokenKind};

use crate::completion::context::PortListKind;

const KEYWORD_VERSION: &str = "1364-2005";

pub(crate) fn port_item_keywords(kind: PortListKind) -> &'static [String] {
    static ANSI_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static FUNCTION_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();

    match kind {
        PortListKind::Ansi => ANSI_KEYWORDS
            .get_or_init(|| port_keywords_matching(SyntaxFacts::is_possible_ansi_port))
            .as_slice(),
        PortListKind::Function => FUNCTION_KEYWORDS
            .get_or_init(|| port_keywords_matching(SyntaxFacts::is_possible_function_port))
            .as_slice(),
        PortListKind::NonAnsi => &[],
    }
}

pub(crate) fn has_port_item_keyword_prefix(prefix: &str, kind: PortListKind) -> bool {
    !prefix.is_empty() && port_item_keywords(kind).iter().any(|keyword| keyword.starts_with(prefix))
}

fn port_keywords_matching(predicate: fn(TokenKind) -> bool) -> Vec<String> {
    let mut keywords = SyntaxToken::keyword_table_for_version(KEYWORD_VERSION)
        .into_iter()
        .filter(|keyword| {
            let kind = SyntaxToken::keyword_kind_for_version(KEYWORD_VERSION, keyword);
            kind != TokenKind::UNKNOWN && predicate(kind)
        })
        .collect::<Vec<_>>();
    keywords.sort();
    keywords.dedup();
    keywords
}
