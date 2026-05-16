use std::sync::OnceLock;

use syntax::{
    SemanticFacts, SyntaxElemPreorder, SyntaxFacts, SyntaxKind, SyntaxNode, SyntaxNodeExt,
    SyntaxNodePreorder, SyntaxToken, TokenKind, WalkEvent,
    ast::{self, AstNode},
};
use utils::line_index::{TextRange, TextSize};

use crate::completion::context::{ExpectedSyntax, PortListKind};

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

pub(crate) fn keyword_candidates(
    expected: ExpectedSyntax,
    source_text: &str,
    replacement: TextRange,
    prefix: &str,
) -> KeywordCandidates {
    if !token_prediction_supported(expected) {
        return KeywordCandidates { labels: Vec::new() };
    }

    KeywordCandidates {
        labels: all_keywords()
            .iter()
            .filter(|keyword| keyword.starts_with(prefix))
            .filter(|keyword| {
                token_prediction_accepts_keyword(expected, source_text, replacement, keyword)
            })
            .cloned()
            .collect(),
    }
}

pub(crate) fn predicts_source_expected_keyword(
    expected: ExpectedSyntax,
    source_text: &str,
    replacement: TextRange,
    prefix: &str,
) -> bool {
    token_prediction_supported(expected)
        && all_keywords().iter().filter(|keyword| keyword.starts_with(prefix)).any(|keyword| {
            token_prediction_accepts_keyword(expected, source_text, replacement, keyword)
        })
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

#[cfg(test)]
fn module_member_keywords() -> &'static [String] {
    static MODULE_MEMBER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    MODULE_MEMBER_KEYWORDS
        .get_or_init(|| {
            let mut keywords = keywords_matching_label(|keyword, _| {
                module_member_keyword_kind(keyword).is_some_and(SyntaxFacts::is_allowed_in_module)
            });
            keywords.extend(gate_type_keywords().iter().cloned());
            keywords.sort();
            keywords.dedup();
            keywords
        })
        .as_slice()
}

#[cfg(test)]
fn generate_member_keywords() -> &'static [String] {
    static GENERATE_MEMBER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    GENERATE_MEMBER_KEYWORDS
        .get_or_init(|| {
            let mut keywords = keywords_matching_label(|keyword, _| {
                generate_member_keyword_kind(keyword)
                    .is_some_and(SyntaxFacts::is_allowed_in_generate)
            });
            keywords.extend(gate_type_keywords().iter().cloned());
            keywords.sort();
            keywords.dedup();
            keywords
        })
        .as_slice()
}

#[cfg(test)]
fn specify_item_keywords() -> &'static [String] {
    static SPECIFY_ITEM_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    SPECIFY_ITEM_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| specify_item_keyword_kind(keyword).is_some())
        })
        .as_slice()
}

#[cfg(test)]
fn config_header_keywords() -> &'static [String] {
    static CONFIG_HEADER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    CONFIG_HEADER_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| config_header_keyword_is_accepted(keyword))
        })
        .as_slice()
}

#[cfg(test)]
fn config_rule_keywords() -> &'static [String] {
    static CONFIG_RULE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    CONFIG_RULE_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| config_rule_keyword_kind(keyword).is_some())
        })
        .as_slice()
}

#[cfg(test)]
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

#[cfg(test)]
fn block_declaration_keywords() -> &'static [String] {
    static BLOCK_DECLARATION_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    BLOCK_DECLARATION_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| {
                block_item_keyword_kind(keyword).is_some_and(|kind| !ast::Statement::can_cast(kind))
            })
        })
        .as_slice()
}

#[cfg(test)]
fn statement_keywords() -> &'static [String] {
    static STATEMENT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    STATEMENT_KEYWORDS.get_or_init(|| keywords_matching(SyntaxFacts::is_possible_statement))
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

fn token_prediction_supported(expected: ExpectedSyntax) -> bool {
    matches!(
        expected,
        ExpectedSyntax::CompilationUnitItem
            | ExpectedSyntax::ModuleHeaderItem
            | ExpectedSyntax::ModuleItem
            | ExpectedSyntax::GenerateItem
            | ExpectedSyntax::SpecifyItem
            | ExpectedSyntax::ConfigItem { .. }
            | ExpectedSyntax::BlockItem { .. }
            | ExpectedSyntax::Statement
            | ExpectedSyntax::ParameterPortListItem
            | ExpectedSyntax::AnsiPortItem
            | ExpectedSyntax::FunctionPortItem
    )
}

fn token_prediction_accepts_keyword(
    expected: ExpectedSyntax,
    source_text: &str,
    replacement: TextRange,
    keyword: &str,
) -> bool {
    let Some(kind) = keyword_kind(keyword) else {
        return false;
    };
    let Some(source) = source_with_replacement(
        source_text,
        replacement,
        &token_prediction_text(expected, keyword),
    ) else {
        return false;
    };
    let start = replacement.start();
    if expected == ExpectedSyntax::CompilationUnitItem {
        return token_prediction_compilation_unit_accepts(&source, start);
    }

    let tree = syntax::SyntaxTree::from_text(&source, "completion-source-probe", "");
    let Some(root) = tree.root() else {
        return false;
    };

    match expected {
        ExpectedSyntax::CompilationUnitItem => unreachable!(),
        ExpectedSyntax::ModuleHeaderItem => {
            root.find_node_at_offset::<ast::ModuleHeader<'_>>(start).is_some_and(|_| {
                started_token_kind_at(root, start, kind) && SyntaxFacts::is_possible_ansi_port(kind)
            })
        }
        ExpectedSyntax::ModuleItem => root
            .find_node_at_offset::<ast::ModuleDeclaration<'_>>(start)
            .and_then(|module| {
                first_started_member_kind_at(
                    module.members().children().map(|member| member.syntax()),
                    start,
                )
            })
            .is_some_and(SyntaxFacts::is_allowed_in_module),
        ExpectedSyntax::GenerateItem => token_prediction_generate_member_kind(root, start)
            .is_some_and(SyntaxFacts::is_allowed_in_generate),
        ExpectedSyntax::SpecifyItem => first_node::<ast::SpecifyBlock<'_>>(root)
            .filter(|specify| {
                node_text_range(specify.syntax())
                    .is_some_and(|range| range.contains(start) || range.start() == start)
            })
            .and_then(|specify| {
                first_started_member_kind_at(
                    specify.items().children().map(|item| item.syntax()),
                    start,
                )
            })
            .is_some(),
        ExpectedSyntax::ConfigItem { rules_allowed: false } => {
            token_prediction_config_header_accepts(root, start)
        }
        ExpectedSyntax::ConfigItem { rules_allowed: true } => root
            .find_node_at_offset::<ast::ConfigDeclaration<'_>>(start)
            .and_then(|config| {
                first_started_member_kind_at(
                    config.rules().children().map(|rule| rule.syntax()),
                    start,
                )
            })
            .is_some(),
        ExpectedSyntax::BlockItem { declarations_allowed } => {
            token_prediction_block_item_kind(root, start)
                .is_some_and(|kind| declarations_allowed || ast::Statement::can_cast(kind))
        }
        ExpectedSyntax::Statement => {
            token_prediction_block_item_kind(root, start).is_some_and(ast::Statement::can_cast)
        }
        ExpectedSyntax::ParameterPortListItem => {
            root.find_node_at_offset::<ast::ParameterPortList<'_>>(start).is_some_and(|_| {
                started_token_kind_at(root, start, kind) && SyntaxFacts::is_possible_parameter(kind)
            })
        }
        ExpectedSyntax::AnsiPortItem => {
            root.find_node_at_offset::<ast::AnsiPortList<'_>>(start).is_some_and(|_| {
                started_token_kind_at(root, start, kind) && SyntaxFacts::is_possible_ansi_port(kind)
            })
        }
        ExpectedSyntax::FunctionPortItem => {
            root.find_node_at_offset::<ast::FunctionPortList<'_>>(start).is_some_and(|_| {
                started_token_kind_at(root, start, kind)
                    && SyntaxFacts::is_possible_function_port(kind)
            })
        }
        _ => false,
    }
}

fn token_prediction_text(_expected: ExpectedSyntax, keyword: &str) -> String {
    format!("{keyword} ")
}

fn source_with_replacement(
    source_text: &str,
    replacement: TextRange,
    replacement_text: &str,
) -> Option<String> {
    let start = usize::from(replacement.start());
    let end = usize::from(replacement.end());
    if start > end
        || end > source_text.len()
        || !source_text.is_char_boundary(start)
        || !source_text.is_char_boundary(end)
    {
        return None;
    }

    let mut source =
        String::with_capacity(source_text.len() - (end - start) + replacement_text.len());
    source.push_str(&source_text[..start]);
    source.push_str(replacement_text);
    source.push_str(&source_text[end..]);
    Some(source)
}

fn token_prediction_compilation_unit_accepts(source: &str, start: TextSize) -> bool {
    let tree = syntax::SyntaxTree::from_text(source, "completion-source-probe", "");
    if let Some(root) = tree.root()
        && let Some(unit) = ast::CompilationUnit::cast(root)
        && first_started_member_kind_at(
            unit.members().children().map(|member| member.syntax()),
            start,
        )
        .is_some_and(SyntaxFacts::is_allowed_in_compilation_unit)
    {
        return true;
    }

    let tree = syntax::SyntaxTree::from_library_map_text(source, "completion-source-probe", "");
    let Some(root) = tree.root() else {
        return false;
    };
    let Some(map) = ast::LibraryMap::cast(root) else {
        return false;
    };
    first_started_member_kind_at(map.members().children().map(|member| member.syntax()), start)
        .is_some()
}

fn token_prediction_generate_member_kind(
    root: SyntaxNode<'_>,
    start: TextSize,
) -> Option<SyntaxKind> {
    if let Some(region) = root.find_node_at_offset::<ast::GenerateRegion<'_>>(start)
        && let Some(kind) = first_started_member_kind_at(
            region.members().children().map(|member| member.syntax()),
            start,
        )
    {
        return Some(kind);
    }

    if let Some(block) = root.find_node_at_offset::<ast::GenerateBlock<'_>>(start) {
        return first_started_member_kind_at(
            block.members().children().map(|member| member.syntax()),
            start,
        );
    }

    None
}

fn token_prediction_block_item_kind(root: SyntaxNode<'_>, start: TextSize) -> Option<SyntaxKind> {
    if let Some(block) = root.find_node_at_offset::<ast::BlockStatement<'_>>(start) {
        if node_text_range(block.syntax()).is_some_and(|range| range.start() == start) {
            return Some(block.syntax().kind());
        }
        if let Some(kind) =
            first_started_member_kind_at(block.items().children().map(|item| item.syntax()), start)
        {
            return Some(kind);
        }
    }

    if let Some(func) = root.find_node_at_offset::<ast::FunctionDeclaration<'_>>(start) {
        return first_started_member_kind_at(
            func.items().children().map(|item| item.syntax()),
            start,
        );
    }

    None
}

fn token_prediction_config_header_accepts(root: SyntaxNode<'_>, start: TextSize) -> bool {
    let Some(config) = root.find_node_at_offset::<ast::ConfigDeclaration<'_>>(start) else {
        return false;
    };

    first_started_member_kind_at(config.localparams().children().map(|param| param.syntax()), start)
        .is_some()
        || config.design().and_then(token_text_range).is_some_and(|range| range.start() == start)
}

#[cfg(test)]
fn module_member_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m;\n";
    let source = format!("{prefix}{keyword} __vizsla;\nendmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let module = first_node::<ast::ModuleDeclaration<'_>>(root)?;
    first_started_member_kind(
        module.members().children().map(|member| member.syntax()),
        prefix.len(),
    )
}

#[cfg(test)]
fn generate_member_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m; generate\n";
    let source = format!("{prefix}{keyword} __vizsla;\nendgenerate endmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let region = first_node::<ast::GenerateRegion<'_>>(root)?;
    first_started_member_kind(
        region.members().children().map(|member| member.syntax()),
        prefix.len(),
    )
}

#[cfg(test)]
fn specify_item_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m; specify\n";
    let source = format!("{prefix}{keyword} __vizsla;\nendspecify endmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let specify = first_node::<ast::SpecifyBlock<'_>>(root)?;
    first_started_member_kind(specify.items().children().map(|item| item.syntax()), prefix.len())
}

#[cfg(test)]
fn config_header_keyword_is_accepted(keyword: &str) -> bool {
    let prefix = "config cfg;\n";
    let source = format!("{prefix}{keyword} __vizsla = 1;\ndesign work.top;\nendconfig\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let Some(root) = tree.root() else {
        return false;
    };
    let Some(config) = first_node::<ast::ConfigDeclaration<'_>>(root) else {
        return false;
    };

    first_started_member_kind(
        config.localparams().children().map(|param| param.syntax()),
        prefix.len(),
    )
    .is_some()
        || config
            .design()
            .and_then(token_text_range)
            .is_some_and(|range| range.start() == TextSize::from(prefix.len() as u32))
}

#[cfg(test)]
fn config_rule_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "config cfg;\ndesign work.top;\n";
    let source = format!("{prefix}{keyword} __vizsla;\nendconfig\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let config = first_node::<ast::ConfigDeclaration<'_>>(root)?;
    first_started_member_kind(config.rules().children().map(|rule| rule.syntax()), prefix.len())
}

#[cfg(test)]
fn block_item_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m; initial begin\n";
    let source = format!("{prefix}{keyword} __vizsla;\nend endmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let block = first_node::<ast::BlockStatement<'_>>(root)?;
    first_started_member_kind(block.items().children().map(|item| item.syntax()), prefix.len())
}

#[cfg(test)]
fn first_started_member_kind<'a>(
    nodes: impl Iterator<Item = SyntaxNode<'a>>,
    start: usize,
) -> Option<SyntaxKind> {
    first_started_member_kind_at(nodes, TextSize::from(start as u32))
}

fn first_started_member_kind_at<'a>(
    mut nodes: impl Iterator<Item = SyntaxNode<'a>>,
    start: TextSize,
) -> Option<SyntaxKind> {
    nodes
        .find(|node| node_text_range(*node).is_some_and(|range| range.start() == start))
        .map(|node| node.kind())
}

fn started_token_kind_at(root: SyntaxNode<'_>, start: TextSize, kind: TokenKind) -> bool {
    SyntaxElemPreorder::new(root).any(|event| match event {
        WalkEvent::Enter(elem) => elem.as_token().is_some_and(|token| {
            token.kind() == kind
                && token_text_range(token).is_some_and(|range| range.start() == start)
        }),
        WalkEvent::Leave(_) => false,
    })
}

fn node_text_range(node: SyntaxNode<'_>) -> Option<TextRange> {
    let range = node.range()?;
    source_range_to_text_range(range)
}

fn token_text_range(token: SyntaxToken<'_>) -> Option<TextRange> {
    let range = token.range()?;
    source_range_to_text_range(range)
}

fn source_range_to_text_range(range: syntax::SourceRange) -> Option<TextRange> {
    let start = range.start();
    let end = range.end();
    if start > end || end > u32::MAX as usize {
        return None;
    }
    Some(TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32)))
}

fn first_node<'a, N: AstNode<'a>>(root: SyntaxNode<'a>) -> Option<N> {
    SyntaxNodePreorder::new(root).find_map(|event| match event {
        WalkEvent::Enter(node) => N::cast(node),
        WalkEvent::Leave(_) => None,
    })
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
    fn module_member_keywords_are_parser_filtered() {
        assert!(module_member_keywords().iter().any(|keyword| keyword == "always"));
        assert!(module_member_keywords().iter().any(|keyword| keyword == "input"));
        assert!(!module_member_keywords().iter().any(|keyword| keyword == "while"));
    }

    #[test]
    fn generate_member_keywords_are_parser_filtered() {
        assert!(generate_member_keywords().iter().any(|keyword| keyword == "begin"));
        assert!(generate_member_keywords().iter().any(|keyword| keyword == "wire"));
        assert!(!generate_member_keywords().iter().any(|keyword| keyword == "while"));
    }

    #[test]
    fn specify_item_keywords_are_parser_filtered() {
        assert!(specify_item_keywords().iter().any(|keyword| keyword == "specparam"));
        assert!(!specify_item_keywords().iter().any(|keyword| keyword == "wire"));
    }

    #[test]
    fn token_prediction_keeps_specify_item_keywords() {
        let source = "module m; specify\n  sp\nendspecify endmodule\n";
        let start = source.find("sp\n").unwrap();
        let replacement = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + "sp".len()) as u32),
        );
        let keywords =
            keyword_candidates(ExpectedSyntax::SpecifyItem, source, replacement, "").into_labels();
        assert!(keywords.iter().any(|keyword| keyword == "specparam"));
    }

    #[test]
    fn token_prediction_predicts_module_item_tokens() {
        let keywords =
            source_keywords_at(ExpectedSyntax::ModuleItem, "module m;\n  /*caret*/\nendmodule\n");

        assert!(keywords.iter().any(|keyword| keyword == "assign"));
        assert!(keywords.iter().any(|keyword| keyword == "always"));
        assert!(keywords.iter().any(|keyword| keyword == "wire"));
        assert!(keywords.iter().any(|keyword| keyword == "buf"));
        assert!(!keywords.iter().any(|keyword| keyword == "while"));
        assert!(!keywords.iter().any(|keyword| keyword == "return"));
        assert!(!keywords.iter().any(|keyword| keyword == "endmodule"));
    }

    #[test]
    fn keyword_candidates_filter_prefix_before_prediction() {
        let source = "module m;\n  al\nendmodule\n";
        let start = source.find("al\n").unwrap();
        let replacement = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + "al".len()) as u32),
        );
        let candidates = keyword_candidates(ExpectedSyntax::ModuleItem, source, replacement, "al");

        assert!(candidates.contains_plain("always"));
        assert!(candidates.labels().iter().all(|keyword| keyword.starts_with("al")));
    }

    #[test]
    fn token_prediction_predicts_module_header_tokens() {
        let keywords = source_keywords_at(
            ExpectedSyntax::ModuleHeaderItem,
            "module m /*caret*/;\nendmodule\n",
        );

        assert!(keywords.iter().any(|keyword| keyword == "input"));
        assert!(keywords.iter().any(|keyword| keyword == "output"));
        assert!(!keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn token_prediction_predicts_generate_item_tokens() {
        let keywords = source_keywords_at(
            ExpectedSyntax::GenerateItem,
            "module m; generate\n  /*caret*/\nendgenerate endmodule\n",
        );

        assert!(keywords.iter().any(|keyword| keyword == "assign"));
        assert!(keywords.iter().any(|keyword| keyword == "begin"));
        assert!(keywords.iter().any(|keyword| keyword == "wire"));
        assert!(keywords.iter().any(|keyword| keyword == "buf"));
        assert!(!keywords.iter().any(|keyword| keyword == "while"));
        assert!(!keywords.iter().any(|keyword| keyword == "return"));
    }

    #[test]
    fn token_prediction_predicts_block_and_statement_tokens() {
        let block_keywords = source_keywords_at(
            ExpectedSyntax::BlockItem { declarations_allowed: true },
            "module m; initial begin\n  /*caret*/\nend endmodule\n",
        );
        assert!(block_keywords.iter().any(|keyword| keyword == "integer"));
        assert!(block_keywords.iter().any(|keyword| keyword == "for"));
        assert!(!block_keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!block_keywords.iter().any(|keyword| keyword == "module"));

        let statement_keywords = source_keywords_at(
            ExpectedSyntax::Statement,
            "module m; initial begin\n  /*caret*/\nend endmodule\n",
        );
        assert!(statement_keywords.iter().any(|keyword| keyword == "for"));
        assert!(statement_keywords.iter().any(|keyword| keyword == "if"));
        assert!(statement_keywords.iter().any(|keyword| keyword == "begin"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "integer"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "wire"));
        assert!(!statement_keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn token_prediction_predicts_port_and_parameter_tokens() {
        let ansi_keywords =
            source_keywords_at(ExpectedSyntax::AnsiPortItem, "module m(/*caret*/);\nendmodule\n");
        assert!(ansi_keywords.iter().any(|keyword| keyword == "input"));
        assert!(ansi_keywords.iter().any(|keyword| keyword == "output"));
        assert!(!ansi_keywords.iter().any(|keyword| keyword == "always"));

        let function_keywords = source_keywords_at(
            ExpectedSyntax::FunctionPortItem,
            "module m; function integer f(/*caret*/); endfunction endmodule\n",
        );
        assert!(function_keywords.iter().any(|keyword| keyword == "input"));
        assert!(function_keywords.iter().any(|keyword| keyword == "output"));
        assert!(!function_keywords.iter().any(|keyword| keyword == "always"));

        let parameter_keywords = source_keywords_at(
            ExpectedSyntax::ParameterPortListItem,
            "module m #(/*caret*/)(); endmodule\n",
        );
        assert!(
            parameter_keywords.iter().any(|keyword| keyword == "parameter"),
            "parameter predictions: {parameter_keywords:?}"
        );
        assert!(!parameter_keywords.iter().any(|keyword| keyword == "always"));
    }

    #[test]
    fn token_prediction_handles_all_keywords_in_core_contexts() {
        let cases = [
            (ExpectedSyntax::CompilationUnitItem, "/*caret*/\n"),
            (ExpectedSyntax::ModuleHeaderItem, "module m /*caret*/;\nendmodule\n"),
            (ExpectedSyntax::ModuleItem, "module m;\n  /*caret*/\nendmodule\n"),
            (
                ExpectedSyntax::GenerateItem,
                "module m; generate\n  /*caret*/\nendgenerate endmodule\n",
            ),
            (ExpectedSyntax::SpecifyItem, "module m; specify\n  /*caret*/\nendspecify endmodule\n"),
            (
                ExpectedSyntax::ConfigItem { rules_allowed: false },
                "config cfg;\n  /*caret*/\n  design work.top;\nendconfig\n",
            ),
            (
                ExpectedSyntax::ConfigItem { rules_allowed: true },
                "config cfg;\n  design work.top;\n  /*caret*/\nendconfig\n",
            ),
            (
                ExpectedSyntax::BlockItem { declarations_allowed: true },
                "module m; initial begin\n  /*caret*/\nend endmodule\n",
            ),
            (ExpectedSyntax::Statement, "module m; initial begin\n  /*caret*/\nend endmodule\n"),
            (ExpectedSyntax::AnsiPortItem, "module m(/*caret*/);\nendmodule\n"),
            (
                ExpectedSyntax::FunctionPortItem,
                "module m; function integer f(/*caret*/); endfunction endmodule\n",
            ),
            (ExpectedSyntax::ParameterPortListItem, "module m #(/*caret*/)(); endmodule\n"),
        ];

        for (expected, text) in cases {
            let keywords = source_keywords_at(expected, text);
            assert!(!keywords.is_empty(), "{expected:?} produced no token predictions");
        }
    }

    #[test]
    fn config_keywords_are_parser_filtered() {
        assert!(config_header_keywords().iter().any(|keyword| keyword == "design"));
        assert!(!config_header_keywords().iter().any(|keyword| keyword == "default"));
        assert!(config_rule_keywords().iter().any(|keyword| keyword == "default"));
        assert!(config_rule_keywords().iter().any(|keyword| keyword == "instance"));
        assert!(config_rule_keywords().iter().any(|keyword| keyword == "cell"));
        assert!(!config_rule_keywords().iter().any(|keyword| keyword == "design"));
    }

    #[test]
    fn block_item_keywords_are_parser_filtered() {
        assert!(block_item_keywords().iter().any(|keyword| keyword == "integer"));
        assert!(block_item_keywords().iter().any(|keyword| keyword == "while"));
        assert!(!block_item_keywords().iter().any(|keyword| keyword == "wire"));
    }

    fn source_keywords_at(expected: ExpectedSyntax, text: &str) -> Vec<String> {
        let caret = text.find("/*caret*/").unwrap();
        let source = text.replace("/*caret*/", "");
        let offset = TextSize::from(caret as u32);
        keyword_candidates(expected, &source, TextRange::empty(offset), "").into_labels()
    }
}
