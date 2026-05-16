use std::sync::OnceLock;

use syntax::{
    SemanticFacts, SyntaxFacts, SyntaxKind, SyntaxNode, SyntaxNodeExt, SyntaxNodePreorder,
    SyntaxToken, TokenKind, WalkEvent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};

use crate::completion::context::{ExpectedSyntax, PortListKind};

const KEYWORD_VERSION: &str = "1364-2005";

pub(crate) fn keywords_for_expected(expected: ExpectedSyntax) -> &'static [String] {
    match expected {
        ExpectedSyntax::CompilationUnitItem => compilation_unit_keywords(),
        ExpectedSyntax::ModuleHeaderItem => module_header_keywords(),
        ExpectedSyntax::ModuleItem => module_member_keywords(),
        ExpectedSyntax::GenerateItem => generate_member_keywords(),
        ExpectedSyntax::SpecifyItem => specify_item_keywords(),
        ExpectedSyntax::ConfigItem { rules_allowed: false } => config_header_keywords(),
        ExpectedSyntax::ConfigItem { rules_allowed: true } => config_rule_keywords(),
        ExpectedSyntax::BlockItem { declarations_allowed: true } => block_item_keywords(),
        ExpectedSyntax::BlockItem { declarations_allowed: false } | ExpectedSyntax::Statement => {
            statement_keywords()
        }
        _ => &[],
    }
}

pub(crate) fn keywords_for_source_expected(
    expected: ExpectedSyntax,
    source_text: &str,
    replacement: TextRange,
) -> Vec<String> {
    let keywords = keywords_for_expected(expected);
    if !source_probe_supported(expected) {
        return keywords.to_vec();
    }

    keywords
        .iter()
        .filter(|keyword| source_probe_accepts_keyword(expected, source_text, replacement, keyword))
        .cloned()
        .collect()
}

pub(crate) fn gate_type_keywords() -> &'static [String] {
    static GATE_TYPE_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    GATE_TYPE_KEYWORDS.get_or_init(|| keywords_matching(SyntaxFacts::is_gate_type)).as_slice()
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

fn compilation_unit_keywords() -> &'static [String] {
    static COMPILATION_UNIT_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    COMPILATION_UNIT_KEYWORDS
        .get_or_init(|| {
            keywords_matching_label(|keyword, _| {
                compilation_unit_keyword_kind(keyword)
                    .is_some_and(SyntaxFacts::is_allowed_in_compilation_unit)
                    || library_map_keyword_kind(keyword).is_some()
            })
        })
        .as_slice()
}

fn module_header_keywords() -> &'static [String] {
    static MODULE_HEADER_KEYWORDS: OnceLock<Vec<String>> = OnceLock::new();
    MODULE_HEADER_KEYWORDS
        .get_or_init(|| keywords_matching(SyntaxFacts::is_possible_ansi_port))
        .as_slice()
}

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
            keywords_matching_label(|keyword, _| config_header_keyword_is_accepted(keyword))
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
            keywords_matching_label(|keyword, _| {
                block_item_keyword_kind(keyword).is_some_and(|kind| !ast::Statement::can_cast(kind))
            })
        })
        .as_slice()
}

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
    let mut keywords = SyntaxToken::keyword_table_for_version(KEYWORD_VERSION)
        .into_iter()
        .filter(|keyword| keyword_kind(keyword).is_some_and(|kind| predicate(keyword, kind)))
        .collect::<Vec<_>>();
    keywords.sort();
    keywords.dedup();
    keywords
}

fn keyword_kind(keyword: &str) -> Option<TokenKind> {
    let kind = SyntaxToken::keyword_kind_for_version(KEYWORD_VERSION, keyword);
    (kind != TokenKind::UNKNOWN).then_some(kind)
}

fn source_probe_supported(expected: ExpectedSyntax) -> bool {
    matches!(
        expected,
        ExpectedSyntax::CompilationUnitItem
            | ExpectedSyntax::ModuleItem
            | ExpectedSyntax::GenerateItem
            | ExpectedSyntax::SpecifyItem
            | ExpectedSyntax::ConfigItem { .. }
            | ExpectedSyntax::BlockItem { .. }
            | ExpectedSyntax::Statement
    )
}

fn source_probe_accepts_keyword(
    expected: ExpectedSyntax,
    source_text: &str,
    replacement: TextRange,
    keyword: &str,
) -> bool {
    let Some(source) =
        source_with_replacement(source_text, replacement, &source_probe_text(expected, keyword))
    else {
        return false;
    };
    let start = replacement.start();
    if expected == ExpectedSyntax::CompilationUnitItem {
        return source_probe_compilation_unit_accepts(&source, start);
    }

    let tree = syntax::SyntaxTree::from_text(&source, "completion-source-probe", "");
    let Some(root) = tree.root() else {
        return false;
    };

    match expected {
        ExpectedSyntax::CompilationUnitItem => unreachable!(),
        ExpectedSyntax::ModuleItem => root
            .find_node_at_offset::<ast::ModuleDeclaration<'_>>(start)
            .and_then(|module| {
                first_started_member_kind_at(
                    module.members().children().map(|member| member.syntax()),
                    start,
                )
            })
            .is_some_and(SyntaxFacts::is_allowed_in_module),
        ExpectedSyntax::GenerateItem => source_probe_generate_member_kind(root, start)
            .is_some_and(SyntaxFacts::is_allowed_in_generate),
        ExpectedSyntax::SpecifyItem => first_node::<ast::SpecifyBlock<'_>>(root)
            .filter(|specify| {
                specify
                    .syntax()
                    .text_range()
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
            source_probe_config_header_accepts(root, start)
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
            source_probe_block_item_kind(root, start)
                .is_some_and(|kind| declarations_allowed || ast::Statement::can_cast(kind))
        }
        ExpectedSyntax::Statement => {
            source_probe_block_item_kind(root, start).is_some_and(ast::Statement::can_cast)
        }
        _ => true,
    }
}

fn source_probe_text(expected: ExpectedSyntax, keyword: &str) -> String {
    match expected {
        ExpectedSyntax::SpecifyItem => format!("{keyword} __vizsla = 1;\n"),
        _ => format!("{keyword} __vizsla;\n"),
    }
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

fn source_probe_compilation_unit_accepts(source: &str, start: TextSize) -> bool {
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

fn source_probe_generate_member_kind(root: SyntaxNode<'_>, start: TextSize) -> Option<SyntaxKind> {
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

fn source_probe_block_item_kind(root: SyntaxNode<'_>, start: TextSize) -> Option<SyntaxKind> {
    if let Some(block) = root.find_node_at_offset::<ast::BlockStatement<'_>>(start)
        && let Some(kind) =
            first_started_member_kind_at(block.items().children().map(|item| item.syntax()), start)
    {
        return Some(kind);
    }

    if let Some(func) = root.find_node_at_offset::<ast::FunctionDeclaration<'_>>(start) {
        return first_started_member_kind_at(
            func.items().children().map(|item| item.syntax()),
            start,
        );
    }

    None
}

fn source_probe_config_header_accepts(root: SyntaxNode<'_>, start: TextSize) -> bool {
    let Some(config) = root.find_node_at_offset::<ast::ConfigDeclaration<'_>>(start) else {
        return false;
    };

    first_started_member_kind_at(config.localparams().children().map(|param| param.syntax()), start)
        .is_some()
        || config
            .design()
            .and_then(|token| token.text_range())
            .is_some_and(|range| range.start() == start)
}

fn compilation_unit_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let source = format!("{keyword} __vizsla;\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let unit = ast::CompilationUnit::cast(root)?;
    first_started_member_kind(unit.members().children().map(|member| member.syntax()), 0)
}

fn library_map_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let source = format!("{keyword} __vizsla;\n");
    let tree = syntax::SyntaxTree::from_library_map_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let map = ast::LibraryMap::cast(root)?;
    first_started_member_kind(map.members().children().map(|member| member.syntax()), 0)
}

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

fn specify_item_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m; specify\n";
    let source = format!("{prefix}{keyword} __vizsla;\nendspecify endmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let specify = first_node::<ast::SpecifyBlock<'_>>(root)?;
    first_started_member_kind(specify.items().children().map(|item| item.syntax()), prefix.len())
}

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
            .and_then(|token| token.text_range())
            .is_some_and(|range| range.start() == TextSize::from(prefix.len() as u32))
}

fn config_rule_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "config cfg;\ndesign work.top;\n";
    let source = format!("{prefix}{keyword} __vizsla;\nendconfig\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let config = first_node::<ast::ConfigDeclaration<'_>>(root)?;
    first_started_member_kind(config.rules().children().map(|rule| rule.syntax()), prefix.len())
}

fn block_item_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m; initial begin\n";
    let source = format!("{prefix}{keyword} __vizsla;\nend endmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let block = first_node::<ast::BlockStatement<'_>>(root)?;
    first_started_member_kind(block.items().children().map(|item| item.syntax()), prefix.len())
}

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
        .find(|node| node.text_range().is_some_and(|range| range.start() == start))
        .map(|node| node.kind())
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
    fn source_probe_keeps_specify_item_keywords() {
        let source = "module m; specify\n  sp\nendspecify endmodule\n";
        let start = source.find("sp\n").unwrap();
        let replacement = TextRange::new(
            TextSize::from(start as u32),
            TextSize::from((start + "sp".len()) as u32),
        );
        let keywords =
            keywords_for_source_expected(ExpectedSyntax::SpecifyItem, source, replacement);
        assert!(keywords.iter().any(|keyword| keyword == "specparam"));
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
}
