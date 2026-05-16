use std::sync::OnceLock;

use syntax::{SemanticFacts, SyntaxFacts, SyntaxKind, SyntaxToken, TokenKind};

use crate::completion::{context::ExpectedSyntax, request::PortListKind};

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

    pub(crate) fn into_labels(self) -> Vec<String> {
        self.labels
    }
}

pub(crate) fn keyword_candidates(expected: ExpectedSyntax, prefix: &str) -> KeywordCandidates {
    KeywordCandidates {
        labels: keywords_for_expected(expected)
            .iter()
            .filter(|keyword| keyword.starts_with(prefix))
            .cloned()
            .collect(),
    }
}

#[cfg(test)]
fn gate_type_keywords() -> &'static [String] {
    static GATE_TYPE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    GATE_TYPE_KEYWORDS.get_or_init(|| keywords_matching(SyntaxFacts::is_gate_type)).as_slice()
}

pub(crate) fn edge_keywords() -> &'static [String] {
    static EDGE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    EDGE_KEYWORDS.get_or_init(|| keywords_matching(SemanticFacts::is_edge_kind)).as_slice()
}

fn keywords_for_expected(expected: ExpectedSyntax) -> &'static [String] {
    match expected {
        ExpectedSyntax::CompilationUnitItem => compilation_unit_keywords(),
        ExpectedSyntax::ModuleHeaderItem => port_item_keywords(PortListKind::Ansi),
        ExpectedSyntax::ModuleItem => module_member_keywords(),
        ExpectedSyntax::GenerateItem => generate_member_keywords(),
        ExpectedSyntax::SpecifyItem => specify_item_keywords(),
        ExpectedSyntax::ConfigItem { rules_allowed: false } => config_header_keywords(),
        ExpectedSyntax::ConfigItem { rules_allowed: true } => config_rule_keywords(),
        ExpectedSyntax::BlockItem { declarations_allowed: true } => block_item_keywords(),
        ExpectedSyntax::BlockItem { declarations_allowed: false } | ExpectedSyntax::Statement => {
            statement_keywords()
        }
        ExpectedSyntax::ParameterPortListItem => parameter_keywords(),
        ExpectedSyntax::AnsiPortItem => port_item_keywords(PortListKind::Ansi),
        ExpectedSyntax::FunctionPortItem => port_item_keywords(PortListKind::Function),
        _ => &[],
    }
}

fn compilation_unit_keywords() -> &'static [String] {
    static COMPILATION_UNIT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    COMPILATION_UNIT_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, kind| {
                member_keyword_kind(keyword, kind)
                    .is_some_and(SyntaxFacts::is_allowed_in_compilation_unit)
                    || library_map_keyword_kind(keyword).is_some()
            })
        })
        .as_slice()
}

fn module_member_keywords() -> &'static [String] {
    static MODULE_MEMBER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    MODULE_MEMBER_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, kind| {
                member_keyword_kind(keyword, kind).is_some_and(SyntaxFacts::is_allowed_in_module)
            })
        })
        .as_slice()
}

fn generate_member_keywords() -> &'static [String] {
    static GENERATE_MEMBER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    GENERATE_MEMBER_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, kind| {
                member_keyword_kind(keyword, kind).is_some_and(SyntaxFacts::is_allowed_in_generate)
            })
        })
        .as_slice()
}

fn specify_item_keywords() -> &'static [String] {
    static SPECIFY_ITEM_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    SPECIFY_ITEM_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| specify_item_keyword_kind(keyword).is_some())
        })
        .as_slice()
}

fn config_header_keywords() -> &'static [String] {
    static CONFIG_HEADER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    CONFIG_HEADER_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| config_header_keyword_kind(keyword).is_some())
        })
        .as_slice()
}

fn config_rule_keywords() -> &'static [String] {
    static CONFIG_RULE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    CONFIG_RULE_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| config_rule_keyword_kind(keyword).is_some())
        })
        .as_slice()
}

fn block_item_keywords() -> &'static [String] {
    static BLOCK_ITEM_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    BLOCK_ITEM_KEYWORDS
        .get_or_init(|| {
            let mut keywords = block_declaration_keywords()
                .iter()
                .chain(statement_keywords().iter())
                .cloned()
                .collect::<Vec<_>>();
            keywords.sort();
            keywords.dedup();
            keywords
        })
        .as_slice()
}

fn block_declaration_keywords() -> &'static [String] {
    static BLOCK_DECLARATION_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    BLOCK_DECLARATION_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, kind| {
                block_declaration_keyword_kind(keyword, kind).is_some()
            })
        })
        .as_slice()
}

fn statement_keywords() -> &'static [String] {
    static STATEMENT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    STATEMENT_KEYWORDS.get_or_init(|| keywords_matching(SyntaxFacts::is_possible_statement))
}

fn parameter_keywords() -> &'static [String] {
    static PARAMETER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    PARAMETER_KEYWORDS.get_or_init(|| keywords_matching(SyntaxFacts::is_possible_parameter))
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

fn member_keyword_kind(keyword: &str, kind: TokenKind) -> Option<SyntaxKind> {
    if let Some(kind) = known_syntax_kind(SyntaxFacts::get_module_declaration_kind(kind)) {
        return Some(kind);
    }

    if let Some(kind) = known_syntax_kind(SyntaxFacts::get_procedural_block_kind(kind)) {
        return Some(kind);
    }

    if SyntaxFacts::is_gate_type(kind) {
        return Some(SyntaxKind::PRIMITIVE_INSTANTIATION);
    }

    if SyntaxFacts::is_port_direction(kind) {
        return Some(SyntaxKind::PORT_DECLARATION);
    }

    if SyntaxFacts::is_net_type(kind) {
        return Some(SyntaxKind::NET_DECLARATION);
    }

    if is_data_declaration_keyword(kind) {
        return Some(SyntaxKind::DATA_DECLARATION);
    }

    match keyword {
        "assign" => Some(SyntaxKind::CONTINUOUS_ASSIGN),
        "begin" => Some(SyntaxKind::GENERATE_BLOCK),
        "case" | "casex" | "casez" => Some(SyntaxKind::CASE_GENERATE),
        "config" => Some(SyntaxKind::CONFIG_DECLARATION),
        "defparam" => Some(SyntaxKind::DEF_PARAM),
        "for" => Some(SyntaxKind::LOOP_GENERATE),
        "function" => Some(SyntaxKind::FUNCTION_DECLARATION),
        "generate" => Some(SyntaxKind::GENERATE_REGION),
        "genvar" => Some(SyntaxKind::GENVAR_DECLARATION),
        "if" => Some(SyntaxKind::IF_GENERATE),
        "localparam" | "parameter" => Some(SyntaxKind::PARAMETER_DECLARATION_STATEMENT),
        "primitive" => Some(SyntaxKind::UDP_DECLARATION),
        "specify" => Some(SyntaxKind::SPECIFY_BLOCK),
        "specparam" => Some(SyntaxKind::SPECPARAM_DECLARATION),
        "task" => Some(SyntaxKind::TASK_DECLARATION),
        _ => None,
    }
}

fn library_map_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    match keyword {
        "include" => Some(SyntaxKind::LIBRARY_INCLUDE_STATEMENT),
        "library" => Some(SyntaxKind::LIBRARY_DECLARATION),
        _ => None,
    }
}

fn specify_item_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    match keyword {
        "if" => Some(SyntaxKind::CONDITIONAL_PATH_DECLARATION),
        "ifnone" => Some(SyntaxKind::IF_NONE_PATH_DECLARATION),
        "noshowcancelled" | "pulsestyle_ondetect" | "pulsestyle_onevent" | "showcancelled" => {
            Some(SyntaxKind::PULSE_STYLE_DECLARATION)
        }
        "specparam" => Some(SyntaxKind::SPECPARAM_DECLARATION),
        _ => None,
    }
}

fn config_header_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    match keyword {
        "design" => Some(SyntaxKind::CONFIG_DECLARATION),
        "localparam" => Some(SyntaxKind::PARAMETER_DECLARATION_STATEMENT),
        _ => None,
    }
}

fn config_rule_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    match keyword {
        "cell" => Some(SyntaxKind::CELL_CONFIG_RULE),
        "default" => Some(SyntaxKind::DEFAULT_CONFIG_RULE),
        "instance" => Some(SyntaxKind::INSTANCE_CONFIG_RULE),
        _ => None,
    }
}

fn block_declaration_keyword_kind(keyword: &str, kind: TokenKind) -> Option<SyntaxKind> {
    match keyword {
        "localparam" | "parameter" => Some(SyntaxKind::PARAMETER_DECLARATION_STATEMENT),
        _ if is_data_declaration_keyword(kind) => Some(SyntaxKind::DATA_DECLARATION),
        _ => None,
    }
}

fn is_data_declaration_keyword(kind: TokenKind) -> bool {
    known_syntax_kind(SyntaxFacts::get_integer_type(kind)).is_some()
        || known_syntax_kind(SyntaxFacts::get_keyword_type(kind)).is_some()
        || SyntaxFacts::is_possible_data_type(kind)
}

fn known_syntax_kind(kind: SyntaxKind) -> Option<SyntaxKind> {
    (kind != SyntaxKind::UNKNOWN).then_some(kind)
}

fn keywords_matching(predicate: fn(TokenKind) -> bool) -> Vec<String> {
    keywords_matching_label(|_, kind| predicate(kind))
}

fn keywords_matching_label(predicate: impl Fn(&str, TokenKind) -> bool) -> Vec<String> {
    let mut keywords = all_keywords()
        .iter()
        .filter(|keyword| keyword_kind(keyword).is_some_and(|kind| predicate(keyword, kind)))
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
    fn compilation_unit_keywords_use_member_and_library_map_facts() {
        let keywords = keywords_at(ExpectedSyntax::CompilationUnitItem);

        assert!(keywords.iter().any(|keyword| keyword == "module"));
        assert!(keywords.iter().any(|keyword| keyword == "macromodule"));
        assert!(keywords.iter().any(|keyword| keyword == "primitive"));
        assert!(keywords.iter().any(|keyword| keyword == "config"));
        assert!(keywords.iter().any(|keyword| keyword == "library"));
        assert!(keywords.iter().any(|keyword| keyword == "include"));
        assert!(!keywords.iter().any(|keyword| keyword == "endmodule"));
    }

    #[test]
    fn module_member_keywords_use_slang_allowed_in_module() {
        let keywords = keywords_at(ExpectedSyntax::ModuleItem);

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
        let keywords = keywords_at(ExpectedSyntax::GenerateItem);

        assert!(keywords.iter().any(|keyword| keyword == "assign"));
        assert!(keywords.iter().any(|keyword| keyword == "begin"));
        assert!(keywords.iter().any(|keyword| keyword == "wire"));
        assert!(keywords.iter().any(|keyword| keyword == "buf"));
        assert!(!keywords.iter().any(|keyword| keyword == "while"));
        assert!(!keywords.iter().any(|keyword| keyword == "return"));
        assert!(!keywords.iter().any(|keyword| keyword == "generate"));
    }

    #[test]
    fn specify_item_keywords_use_specify_item_entries() {
        let keywords = keywords_at(ExpectedSyntax::SpecifyItem);

        assert!(keywords.iter().any(|keyword| keyword == "specparam"));
        assert!(keywords.iter().any(|keyword| keyword == "pulsestyle_ondetect"));
        assert!(keywords.iter().any(|keyword| keyword == "ifnone"));
        assert!(keywords.iter().any(|keyword| keyword == "if"));
        assert!(!keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!keywords.iter().any(|keyword| keyword == "specify"));
    }

    #[test]
    fn keyword_candidates_filter_prefix() {
        let candidates = keyword_candidates(ExpectedSyntax::ModuleItem, "al");

        assert!(candidates.contains_plain("always"));
        assert!(candidates.labels().iter().all(|keyword| keyword.starts_with("al")));
    }

    #[test]
    fn module_header_keywords_use_ansi_port_facts() {
        let keywords = keywords_at(ExpectedSyntax::ModuleHeaderItem);

        assert!(keywords.iter().any(|keyword| keyword == "input"));
        assert!(keywords.iter().any(|keyword| keyword == "output"));
        assert!(!keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn block_and_statement_keywords_are_separate() {
        let block_keywords = keywords_at(ExpectedSyntax::BlockItem { declarations_allowed: true });
        assert!(block_keywords.iter().any(|keyword| keyword == "integer"));
        assert!(block_keywords.iter().any(|keyword| keyword == "localparam"));
        assert!(block_keywords.iter().any(|keyword| keyword == "for"));
        assert!(!block_keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!block_keywords.iter().any(|keyword| keyword == "module"));

        let statement_keywords = keywords_at(ExpectedSyntax::Statement);
        assert!(statement_keywords.iter().any(|keyword| keyword == "for"));
        assert!(statement_keywords.iter().any(|keyword| keyword == "if"));
        assert!(statement_keywords.iter().any(|keyword| keyword == "begin"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "integer"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn port_and_parameter_keywords_use_slang_token_facts() {
        let ansi_keywords = keywords_at(ExpectedSyntax::AnsiPortItem);
        assert!(ansi_keywords.iter().any(|keyword| keyword == "input"));
        assert!(ansi_keywords.iter().any(|keyword| keyword == "output"));
        assert!(!ansi_keywords.iter().any(|keyword| keyword == "always"));

        let function_keywords = keywords_at(ExpectedSyntax::FunctionPortItem);
        assert!(function_keywords.iter().any(|keyword| keyword == "input"));
        assert!(function_keywords.iter().any(|keyword| keyword == "output"));
        assert!(!function_keywords.iter().any(|keyword| keyword == "always"));

        let parameter_keywords = keywords_at(ExpectedSyntax::ParameterPortListItem);
        assert!(
            parameter_keywords.iter().any(|keyword| keyword == "parameter"),
            "parameter predictions: {parameter_keywords:?}"
        );
        assert!(!parameter_keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn core_keyword_contexts_are_non_empty() {
        let cases = [
            ExpectedSyntax::CompilationUnitItem,
            ExpectedSyntax::ModuleHeaderItem,
            ExpectedSyntax::ModuleItem,
            ExpectedSyntax::GenerateItem,
            ExpectedSyntax::SpecifyItem,
            ExpectedSyntax::ConfigItem { rules_allowed: false },
            ExpectedSyntax::ConfigItem { rules_allowed: true },
            ExpectedSyntax::BlockItem { declarations_allowed: true },
            ExpectedSyntax::Statement,
            ExpectedSyntax::AnsiPortItem,
            ExpectedSyntax::FunctionPortItem,
            ExpectedSyntax::ParameterPortListItem,
        ];

        for expected in cases {
            let keywords = keywords_at(expected);
            assert!(!keywords.is_empty(), "{expected:?} produced no keywords");
        }
    }

    #[test]
    fn config_keywords_use_config_phase_entries() {
        let header = config_header_keywords();
        assert!(header.iter().any(|keyword| keyword == "design"));
        assert!(header.iter().any(|keyword| keyword == "localparam"));
        assert!(!header.iter().any(|keyword| keyword == "default"));

        let rules = config_rule_keywords();
        assert!(rules.iter().any(|keyword| keyword == "default"));
        assert!(rules.iter().any(|keyword| keyword == "instance"));
        assert!(rules.iter().any(|keyword| keyword == "cell"));
        assert!(!rules.iter().any(|keyword| keyword == "design"));
    }

    fn keywords_at(expected: ExpectedSyntax) -> Vec<String> {
        keyword_candidates(expected, "").into_labels()
    }
}
