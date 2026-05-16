use std::sync::OnceLock;

use syntax::{
    SemanticFacts, SyntaxFacts, SyntaxKind, SyntaxNode, SyntaxNodePreorder, SyntaxToken, TokenKind,
    WalkEvent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::line_index::TextSize;

use crate::completion::context::{ExpectedSyntax, PortListKind};

const KEYWORD_VERSION: &str = "1364-2005";

pub(crate) fn keywords_for_expected(expected: ExpectedSyntax) -> &'static [String] {
    match expected {
        ExpectedSyntax::CompilationUnitItem => compilation_unit_keywords(),
        ExpectedSyntax::ModuleHeaderItem => module_header_keywords(),
        ExpectedSyntax::ModuleItem => module_member_keywords(),
        ExpectedSyntax::BlockItem { declarations_allowed: true } => block_item_keywords(),
        ExpectedSyntax::BlockItem { declarations_allowed: false } | ExpectedSyntax::Statement => {
            statement_keywords()
        }
        _ => &[],
    }
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

fn block_item_keyword_kind(keyword: &str) -> Option<SyntaxKind> {
    let prefix = "module m; initial begin\n";
    let source = format!("{prefix}{keyword} __vizsla;\nend endmodule\n");
    let tree = syntax::SyntaxTree::from_text(&source, "completion-probe", "");
    let root = tree.root()?;
    let block = first_node::<ast::BlockStatement<'_>>(root)?;
    first_started_member_kind(block.items().children().map(|item| item.syntax()), prefix.len())
}

fn first_started_member_kind<'a>(
    mut nodes: impl Iterator<Item = SyntaxNode<'a>>,
    start: usize,
) -> Option<SyntaxKind> {
    let start = TextSize::from(start as u32);
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
    fn block_item_keywords_are_parser_filtered() {
        assert!(block_item_keywords().iter().any(|keyword| keyword == "integer"));
        assert!(block_item_keywords().iter().any(|keyword| keyword == "while"));
        assert!(!block_item_keywords().iter().any(|keyword| keyword == "wire"));
    }
}
