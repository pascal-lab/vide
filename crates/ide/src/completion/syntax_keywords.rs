use std::sync::OnceLock;

use syntax::{SemanticFacts, SyntaxFacts, SyntaxToken, TokenKind};

use crate::completion::context::PortListKind;

const KEYWORD_VERSION: &str = "1364-2005";

pub(crate) fn gate_type_keywords() -> &'static [String] {
    static GATE_TYPE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    GATE_TYPE_KEYWORDS.get_or_init(|| keywords_matching(SyntaxFacts::is_gate_type)).as_slice()
}

pub(crate) fn is_gate_type_keyword(label: &str) -> bool {
    gate_type_keywords().iter().any(|keyword| keyword == label)
}

pub(crate) fn edge_keywords() -> &'static [String] {
    static EDGE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    EDGE_KEYWORDS.get_or_init(|| keywords_matching(SemanticFacts::is_edge_kind)).as_slice()
}

pub(crate) fn parameter_port_keywords() -> &'static [String] {
    static PARAMETER_PORT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    PARAMETER_PORT_KEYWORDS
        .get_or_init(|| keywords_matching(SyntaxFacts::is_possible_parameter))
        .as_slice()
}

pub(crate) fn port_item_keywords(kind: PortListKind) -> &'static [String] {
    static ANSI_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static FUNCTION_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();

    match kind {
        PortListKind::Ansi => ANSI_KEYWORDS
            .get_or_init(|| keywords_matching(SyntaxFacts::is_possible_ansi_port))
            .as_slice(),
        PortListKind::Function => FUNCTION_KEYWORDS
            .get_or_init(|| keywords_matching(SyntaxFacts::is_possible_function_port))
            .as_slice(),
        PortListKind::NonAnsi => &[],
    }
}

pub(crate) fn has_port_item_keyword_prefix(prefix: &str, kind: PortListKind) -> bool {
    !prefix.is_empty() && port_item_keywords(kind).iter().any(|keyword| keyword.starts_with(prefix))
}

fn keywords_matching(predicate: fn(TokenKind) -> bool) -> Vec<String> {
    let mut keywords = SyntaxToken::keyword_table_for_version(KEYWORD_VERSION)
        .into_iter()
        .filter(|keyword| keyword_kind(keyword).is_some_and(predicate))
        .collect::<Vec<_>>();
    keywords.sort();
    keywords.dedup();
    keywords
}

fn keyword_kind(keyword: &str) -> Option<TokenKind> {
    let kind = SyntaxToken::keyword_kind_for_version(KEYWORD_VERSION, keyword);
    (kind != TokenKind::UNKNOWN).then_some(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameter_port_keywords_include_slang_parameter_facts() {
        assert!(parameter_port_keywords().iter().any(|keyword| keyword == "reg"));
    }

    #[test]
    fn gate_type_keywords_include_slang_gate_facts() {
        assert!(gate_type_keywords().iter().any(|keyword| keyword == "bufif0"));
        assert!(gate_type_keywords().iter().any(|keyword| keyword == "and"));
    }

    #[test]
    fn edge_keywords_include_slang_edge_facts() {
        assert!(edge_keywords().iter().any(|keyword| keyword == "posedge"));
        assert!(edge_keywords().iter().any(|keyword| keyword == "negedge"));
        assert!(edge_keywords().iter().any(|keyword| keyword == "edge"));
    }
}
