use std::sync::OnceLock;

use syntax::{SemanticFacts, SyntaxFacts, SyntaxKeywordContext, SyntaxToken, TokenKind};

use crate::completion::request::PortListKind;

const KEYWORD_VERSION: &str = "1364-2005";

#[derive(Debug, Clone)]
pub(crate) struct KeywordCandidates {
    labels: Vec<String>,
}

impl KeywordCandidates {
    pub(crate) fn labels(&self) -> &[String] {
        &self.labels
    }

    pub(crate) fn contains_plain(&self, plain: &str) -> bool {
        self.labels.iter().any(|label| label == plain)
    }

    #[cfg(test)]
    pub(crate) fn into_labels(self) -> Vec<String> {
        self.labels
    }
}

pub(crate) fn keyword_candidates_for_context(
    context: SyntaxKeywordContext,
    prefix: &str,
) -> KeywordCandidates {
    KeywordCandidates {
        labels: keywords_for_context(context)
            .iter()
            .filter(|keyword| keyword.starts_with(prefix))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
fn gate_type_keywords() -> &'static [String] {
    keywords_for_context(SyntaxKeywordContext::GateType)
}

pub(crate) fn edge_keywords() -> &'static [String] {
    static EDGE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    EDGE_KEYWORDS.get_or_init(|| keywords_matching(SemanticFacts::is_edge_kind)).as_slice()
}

fn keywords_for_context(context: SyntaxKeywordContext) -> &'static [String] {
    static COMPILATION_UNIT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static LIBRARY_MAP_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static MODULE_HEADER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static MODULE_MEMBER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static GENERATE_MEMBER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static SPECIFY_ITEM_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static CONFIG_HEADER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static CONFIG_RULE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static BLOCK_ITEM_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static STATEMENT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static PARAMETER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static ANSI_PORT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static FUNCTION_PORT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    static GATE_TYPE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();

    match context {
        SyntaxKeywordContext::CompilationUnitMember => &COMPILATION_UNIT_KEYWORDS,
        SyntaxKeywordContext::LibraryMapMember => &LIBRARY_MAP_KEYWORDS,
        SyntaxKeywordContext::ModuleHeaderItem => &MODULE_HEADER_KEYWORDS,
        SyntaxKeywordContext::ModuleMember => &MODULE_MEMBER_KEYWORDS,
        SyntaxKeywordContext::GenerateMember => &GENERATE_MEMBER_KEYWORDS,
        SyntaxKeywordContext::SpecifyItem => &SPECIFY_ITEM_KEYWORDS,
        SyntaxKeywordContext::ConfigHeaderItem => &CONFIG_HEADER_KEYWORDS,
        SyntaxKeywordContext::ConfigRule => &CONFIG_RULE_KEYWORDS,
        SyntaxKeywordContext::BlockItem => &BLOCK_ITEM_KEYWORDS,
        SyntaxKeywordContext::Statement => &STATEMENT_KEYWORDS,
        SyntaxKeywordContext::ParameterPortListItem => &PARAMETER_KEYWORDS,
        SyntaxKeywordContext::AnsiPortItem => &ANSI_PORT_KEYWORDS,
        SyntaxKeywordContext::FunctionPortItem => &FUNCTION_PORT_KEYWORDS,
        SyntaxKeywordContext::GateType => &GATE_TYPE_KEYWORDS,
    }
    .get_or_init(|| keyword_context_candidates(context))
    .as_slice()
}

pub(crate) fn port_item_keywords(kind: PortListKind) -> &'static [String] {
    match kind {
        PortListKind::Ansi => keywords_for_context(SyntaxKeywordContext::AnsiPortItem),
        PortListKind::Function => keywords_for_context(SyntaxKeywordContext::FunctionPortItem),
        PortListKind::NonAnsi => &[],
    }
}

pub(crate) fn has_port_item_keyword_prefix(prefix: &str, kind: PortListKind) -> bool {
    !prefix.is_empty() && port_item_keywords(kind).iter().any(|keyword| keyword.starts_with(prefix))
}

fn keyword_context_candidates(context: SyntaxKeywordContext) -> Vec<String> {
    SyntaxFacts::keyword_candidates_for_context(KEYWORD_VERSION, context)
}

fn keywords_matching(predicate: impl Fn(TokenKind) -> bool) -> Vec<String> {
    let mut keywords = all_keywords()
        .iter()
        .filter(|keyword| keyword_kind(keyword).is_some_and(&predicate))
        .cloned()
        .collect::<Vec<_>>();
    keywords.sort();
    keywords.dedup();
    keywords
}

fn all_keywords() -> &'static [String] {
    static ALL_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    ALL_KEYWORDS
        .get_or_init(|| {
            let mut keywords = SyntaxToken::keyword_table_for_version(KEYWORD_VERSION);
            keywords.sort();
            keywords.dedup();
            keywords
        })
        .as_slice()
}

fn keyword_kind(keyword: &str) -> Option<TokenKind> {
    let kind = SyntaxToken::keyword_kind_for_version(KEYWORD_VERSION, keyword);
    (kind != TokenKind::UNKNOWN).then_some(kind)
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn keyword_context_matrix() {
        let mut report = String::new();

        writeln!(&mut report, "# special keyword groups").unwrap();
        write_keywords(&mut report, "GateType", gate_type_keywords());
        write_keywords(&mut report, "Edge", edge_keywords());

        writeln!(&mut report, "\n# syntax keyword contexts").unwrap();
        for context in [
            SyntaxKeywordContext::CompilationUnitMember,
            SyntaxKeywordContext::LibraryMapMember,
            SyntaxKeywordContext::ModuleHeaderItem,
            SyntaxKeywordContext::ModuleMember,
            SyntaxKeywordContext::GenerateMember,
            SyntaxKeywordContext::SpecifyItem,
            SyntaxKeywordContext::ConfigHeaderItem,
            SyntaxKeywordContext::ConfigRule,
            SyntaxKeywordContext::BlockItem,
            SyntaxKeywordContext::Statement,
            SyntaxKeywordContext::AnsiPortItem,
            SyntaxKeywordContext::FunctionPortItem,
            SyntaxKeywordContext::ParameterPortListItem,
            SyntaxKeywordContext::GateType,
        ] {
            write_keywords(&mut report, &format!("{context:?}"), &keywords_at(context));
        }

        writeln!(&mut report, "\n# prefix filters").unwrap();
        for (context, prefix) in [
            (SyntaxKeywordContext::ModuleMember, "al"),
            (SyntaxKeywordContext::ModuleMember, "lo"),
            (SyntaxKeywordContext::GenerateMember, "as"),
            (SyntaxKeywordContext::SpecifyItem, "sp"),
            (SyntaxKeywordContext::ConfigHeaderItem, "de"),
            (SyntaxKeywordContext::ConfigRule, "de"),
            (SyntaxKeywordContext::BlockItem, "re"),
            (SyntaxKeywordContext::Statement, "re"),
            (SyntaxKeywordContext::AnsiPortItem, "wir"),
            (SyntaxKeywordContext::ParameterPortListItem, "para"),
        ] {
            let candidates = keyword_candidates_for_context(context, prefix);
            writeln!(
                &mut report,
                "{context:?} prefix {prefix:?}: all_match={} {:?}",
                candidates.labels().iter().all(|keyword| keyword.starts_with(prefix)),
                candidates.labels()
            )
            .unwrap();
        }

        insta::assert_snapshot!(report);
    }

    fn write_keywords(out: &mut String, name: &str, keywords: &[String]) {
        writeln!(out, "{name}:").unwrap();
        for keyword in keywords {
            writeln!(out, "  {keyword}").unwrap();
        }
    }

    fn keywords_at(context: SyntaxKeywordContext) -> Vec<String> {
        keyword_candidates_for_context(context, "").into_labels()
    }
}
