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
        .filter(|keyword| keyword_kind(keyword).is_some_and(|kind| predicate(kind)))
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
    use super::*;

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

    #[test]
    fn compilation_unit_keywords_use_member_facts() {
        let keywords = keywords_at(SyntaxKeywordContext::CompilationUnitMember);

        assert!(keywords.iter().any(|keyword| keyword == "module"));
        assert!(keywords.iter().any(|keyword| keyword == "macromodule"));
        assert!(keywords.iter().any(|keyword| keyword == "primitive"));
        assert!(keywords.iter().any(|keyword| keyword == "config"));
        assert!(!keywords.iter().any(|keyword| keyword == "library"));
        assert!(!keywords.iter().any(|keyword| keyword == "include"));
        assert!(!keywords.iter().any(|keyword| keyword == "endmodule"));
    }

    #[test]
    fn library_map_keywords_use_library_map_facts() {
        let keywords = keywords_at(SyntaxKeywordContext::LibraryMapMember);

        assert!(keywords.iter().any(|keyword| keyword == "library"));
        assert!(keywords.iter().any(|keyword| keyword == "include"));
        assert!(keywords.iter().any(|keyword| keyword == "config"));
        assert!(!keywords.iter().any(|keyword| keyword == "module"));
    }

    #[test]
    fn module_member_keywords_use_slang_allowed_in_module() {
        let keywords = keywords_at(SyntaxKeywordContext::ModuleMember);

        assert!(keywords.iter().any(|keyword| keyword == "always"));
        assert!(keywords.iter().any(|keyword| keyword == "begin"));
        assert!(keywords.iter().any(|keyword| keyword == "input"));
        assert!(keywords.iter().any(|keyword| keyword == "wire"));
        assert!(keywords.iter().any(|keyword| keyword == "localparam"));
        assert!(keywords.iter().any(|keyword| keyword == "buf"));
        assert!(!keywords.iter().any(|keyword| keyword == "while"));
        assert!(!keywords.iter().any(|keyword| keyword == "return"));
        assert!(!keywords.iter().any(|keyword| keyword == "endmodule"));
    }

    #[test]
    fn generate_member_keywords_use_slang_allowed_in_generate() {
        let keywords = keywords_at(SyntaxKeywordContext::GenerateMember);

        assert!(keywords.iter().any(|keyword| keyword == "assign"));
        assert!(keywords.iter().any(|keyword| keyword == "begin"));
        assert!(keywords.iter().any(|keyword| keyword == "wire"));
        assert!(keywords.iter().any(|keyword| keyword == "buf"));
        assert!(!keywords.iter().any(|keyword| keyword == "casex"));
        assert!(!keywords.iter().any(|keyword| keyword == "casez"));
        assert!(!keywords.iter().any(|keyword| keyword == "while"));
        assert!(!keywords.iter().any(|keyword| keyword == "return"));
        assert!(!keywords.iter().any(|keyword| keyword == "generate"));
    }

    #[test]
    fn specify_item_keywords_use_specify_item_entries() {
        let keywords = keywords_at(SyntaxKeywordContext::SpecifyItem);

        assert!(keywords.iter().any(|keyword| keyword == "specparam"));
        assert!(keywords.iter().any(|keyword| keyword == "pulsestyle_ondetect"));
        assert!(keywords.iter().any(|keyword| keyword == "ifnone"));
        assert!(keywords.iter().any(|keyword| keyword == "if"));
        assert!(!keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!keywords.iter().any(|keyword| keyword == "specify"));
    }

    #[test]
    fn keyword_candidates_filter_prefix() {
        let candidates = keyword_candidates_for_context(SyntaxKeywordContext::ModuleMember, "al");

        assert!(candidates.contains_plain("always"));
        assert!(candidates.labels().iter().all(|keyword| keyword.starts_with("al")));
    }

    #[test]
    fn module_header_keywords_use_ansi_port_facts() {
        let keywords = keywords_at(SyntaxKeywordContext::ModuleHeaderItem);

        assert!(keywords.iter().any(|keyword| keyword == "input"));
        assert!(keywords.iter().any(|keyword| keyword == "output"));
        assert!(!keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn block_and_statement_keywords_are_separate() {
        let block_keywords = keywords_at(SyntaxKeywordContext::BlockItem);
        assert!(block_keywords.iter().any(|keyword| keyword == "integer"));
        assert!(block_keywords.iter().any(|keyword| keyword == "localparam"));
        assert!(block_keywords.iter().any(|keyword| keyword == "for"));
        assert!(!block_keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!block_keywords.iter().any(|keyword| keyword == "module"));

        let statement_keywords = keywords_at(SyntaxKeywordContext::Statement);
        assert!(statement_keywords.iter().any(|keyword| keyword == "for"));
        assert!(statement_keywords.iter().any(|keyword| keyword == "if"));
        assert!(statement_keywords.iter().any(|keyword| keyword == "begin"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "integer"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn port_and_parameter_keywords_use_slang_token_facts() {
        let ansi_keywords = keywords_at(SyntaxKeywordContext::AnsiPortItem);
        assert!(ansi_keywords.iter().any(|keyword| keyword == "input"));
        assert!(ansi_keywords.iter().any(|keyword| keyword == "output"));
        assert!(!ansi_keywords.iter().any(|keyword| keyword == "always"));

        let function_keywords = keywords_at(SyntaxKeywordContext::FunctionPortItem);
        assert!(function_keywords.iter().any(|keyword| keyword == "input"));
        assert!(function_keywords.iter().any(|keyword| keyword == "output"));
        assert!(!function_keywords.iter().any(|keyword| keyword == "always"));

        let parameter_keywords = keywords_at(SyntaxKeywordContext::ParameterPortListItem);
        assert!(
            parameter_keywords.iter().any(|keyword| keyword == "parameter"),
            "parameter predictions: {parameter_keywords:?}"
        );
        assert!(!parameter_keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn core_keyword_contexts_are_non_empty() {
        let cases = [
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
        ];

        for context in cases {
            let keywords = keywords_at(context);
            assert!(!keywords.is_empty(), "{context:?} produced no keywords");
        }
    }

    #[test]
    fn config_keywords_use_config_phase_entries() {
        let header = keywords_for_context(SyntaxKeywordContext::ConfigHeaderItem);
        assert!(header.iter().any(|keyword| keyword == "design"));
        assert!(header.iter().any(|keyword| keyword == "localparam"));
        assert!(!header.iter().any(|keyword| keyword == "default"));

        let rules = keywords_for_context(SyntaxKeywordContext::ConfigRule);
        assert!(rules.iter().any(|keyword| keyword == "default"));
        assert!(rules.iter().any(|keyword| keyword == "instance"));
        assert!(rules.iter().any(|keyword| keyword == "cell"));
        assert!(!rules.iter().any(|keyword| keyword == "design"));
    }

    fn keywords_at(context: SyntaxKeywordContext) -> Vec<String> {
        keyword_candidates_for_context(context, "").into_labels()
    }
}
