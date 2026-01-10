use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxCursorExt, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent,
    SyntaxTrivia, TokenKind,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
    token::SyntaxTokenExt,
};
use utils::line_index::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexContext {
    Code,
    LineComment,
    BlockComment,
    StringLiteral,
    PreprocDirective,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynContext {
    TopLevel,
    ModuleHeader,
    ModuleItem,
    Instantiation,
    HierRef,
    SensitivityList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotKind {
    NamedPort,
    NamedParam,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashKind {
    ParamValueAssignment,
    ParameterPortList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParenListKind {
    ParamValueAssignment,
    ParameterPortList,
    PortConnections,
    Arguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AfterDot {
    pub kind: DotKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AfterHash {
    pub kind: HashKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InParenList {
    pub kind: ParenListKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualifier {
    AfterDot(AfterDot),
    AfterHash(AfterHash),
    InParenList(InParenList),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub replacement: TextRange,
    pub prefix: String,
    pub lex: LexContext,
    pub syn: SynContext,
    pub qualifier: Option<Qualifier>,
}

pub(crate) fn completion_context(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
) -> CompletionContext {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);
    detect_completion_context(file.syntax(), offset)
}

pub fn detect_completion_context(root: SyntaxNode<'_>, offset: TextSize) -> CompletionContext {
    let (replacement, prefix) = replacement_and_prefix(root, offset);

    let lex = detect_lex_context(root, offset);
    if lex != LexContext::Code {
        return CompletionContext {
            replacement,
            prefix,
            lex,
            syn: SynContext::TopLevel,
            qualifier: None,
        };
    }

    let (syn, qualifier) = detect_syn_context(root, offset);
    CompletionContext { replacement, prefix, lex, syn, qualifier }
}


fn replacement_and_prefix(root: SyntaxNode<'_>, offset: TextSize) -> (TextRange, String) {
    let token_at = root.token_at_offset(offset);
    let tok_with_parent = match token_at {
        syntax::TokenAtOffset::Single(tok) => Some(tok),
        syntax::TokenAtOffset::Between(left, right) => {
            let left_range = left.text_range();
            if left_range.is_some_and(|r| r.end() == offset) {
                Some(left)
            } else {
                let right_range = right.text_range();
                right_range.is_some_and(|r| r.start() == offset).then_some(right)
            }
        }
        syntax::TokenAtOffset::None => None,
    };

    let Some(tok_with_parent) = tok_with_parent else {
        return (TextRange::empty(offset), String::new());
    };

    match tok_with_parent.kind() {
        TokenKind::IDENTIFIER | TokenKind::SYSTEM_IDENTIFIER => {
            let range = tok_with_parent.text_range().unwrap_or_else(|| TextRange::empty(offset));
            let prefix = if range.contains(offset) || range.end() == offset {
                let upto = usize::from(offset - range.start());
                let text = tok_with_parent.tok.raw_text().to_string();
                if upto <= text.len() && text.is_char_boundary(upto) {
                    text[..upto].to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            (range, prefix)
        }
        _ => (TextRange::empty(offset), String::new()),
    }
}

fn detect_lex_context(root: SyntaxNode<'_>, offset: TextSize) -> LexContext {
    if is_inside_string_literal(root, offset) {
        return LexContext::StringLiteral;
    }

    if let Some(trivia) = trivia_at_offset(root, offset) {
        return match trivia.kind() {
            syntax::Trivia![lc] => LexContext::LineComment,
            syntax::Trivia![bc] => LexContext::BlockComment,
            syntax::Trivia!["`"] => LexContext::PreprocDirective,
            _ => LexContext::Code,
        };
    }

    if is_inside_preproc_directive_node(root, offset) {
        return LexContext::PreprocDirective;
    }

    LexContext::Code
}


fn is_inside_string_literal(root: SyntaxNode<'_>, offset: TextSize) -> bool {
    let tok = root.token_at_offset(offset).left_biased();
    tok.is_some_and(|tp| {
        tp.kind() == TokenKind::STRING_LITERAL
            && tp.text_range().is_some_and(|r| r.contains(offset))
    })
}

fn trivia_at_offset(root: SyntaxNode<'_>, offset: TextSize) -> Option<SyntaxTrivia<'_>> {
    let tok = token_after_or_at_offset(root, offset)?;
    for (range, trivia) in tok.tok.trivias_with_range() {
        if range.contains(offset) {
            return Some(trivia);
        }
    }
    None
}

fn token_after_or_at_offset(
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<SyntaxTokenWithParent<'_>> {
    if let Some(tok) = root.token_at_offset(offset).left_biased()
        && tok.text_range().is_some_and(|r| r.contains(offset))
    {
        return Some(tok);
    }

    let mut cursor = root.walk();
    if !cursor.goto_first_tok_after_or_last(offset) {
        return None;
    }
    cursor.to_tok_with_parent()
}

fn is_inside_preproc_directive_node(root: SyntaxNode<'_>, offset: TextSize) -> bool {
    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return false;
    };

    SyntaxAncestors::start_from(node).any(|n| is_preproc_directive_kind(n.kind()))
}

fn is_preproc_directive_kind(kind: syntax::SyntaxKind) -> bool {
    use syntax::SyntaxKind;
    matches!(
        kind,
        SyntaxKind::BEGIN_KEYWORDS_DIRECTIVE
            | SyntaxKind::CELL_DEFINE_DIRECTIVE
            | SyntaxKind::DEFAULT_DECAY_TIME_DIRECTIVE
            | SyntaxKind::DEFAULT_NET_TYPE_DIRECTIVE
            | SyntaxKind::DEFAULT_TRIREG_STRENGTH_DIRECTIVE
            | SyntaxKind::DEFINE_DIRECTIVE
            | SyntaxKind::DELAY_MODE_DISTRIBUTED_DIRECTIVE
            | SyntaxKind::DELAY_MODE_PATH_DIRECTIVE
            | SyntaxKind::DELAY_MODE_UNIT_DIRECTIVE
            | SyntaxKind::DELAY_MODE_ZERO_DIRECTIVE
            | SyntaxKind::ELSE_DIRECTIVE
            | SyntaxKind::ELS_IF_DIRECTIVE
            | SyntaxKind::END_CELL_DEFINE_DIRECTIVE
            | SyntaxKind::END_IF_DIRECTIVE
            | SyntaxKind::END_KEYWORDS_DIRECTIVE
            | SyntaxKind::END_PROTECT_DIRECTIVE
            | SyntaxKind::END_PROTECTED_DIRECTIVE
            | SyntaxKind::IF_DEF_DIRECTIVE
            | SyntaxKind::IF_N_DEF_DIRECTIVE
            | SyntaxKind::INCLUDE_DIRECTIVE
            | SyntaxKind::LINE_DIRECTIVE
            | SyntaxKind::NO_UNCONNECTED_DRIVE_DIRECTIVE
            | SyntaxKind::PRAGMA_DIRECTIVE
            | SyntaxKind::PROTECT_DIRECTIVE
            | SyntaxKind::PROTECTED_DIRECTIVE
            | SyntaxKind::RESET_ALL_DIRECTIVE
            | SyntaxKind::TIME_SCALE_DIRECTIVE
            | SyntaxKind::UNCONNECTED_DRIVE_DIRECTIVE
            | SyntaxKind::UNDEF_DIRECTIVE
            | SyntaxKind::UNDEFINE_ALL_DIRECTIVE
            | SyntaxKind::BIND_DIRECTIVE
    )
}

fn detect_syn_context(root: SyntaxNode<'_>, offset: TextSize) -> (SynContext, Option<Qualifier>) {
    if let Some(qualifier) = qualifier_after_dot(root, offset) {
        let syn = match qualifier {
            Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort | DotKind::NamedParam }) => {
                SynContext::Instantiation
            }
            Qualifier::AfterDot(AfterDot { kind: DotKind::Member }) => SynContext::HierRef,
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if let Some(qualifier) = qualifier_after_hash(root, offset) {
        let syn = match qualifier {
            Qualifier::AfterHash(AfterHash { kind: HashKind::ParamValueAssignment }) => {
                SynContext::Instantiation
            }
            Qualifier::AfterHash(AfterHash { kind: HashKind::ParameterPortList }) => {
                SynContext::ModuleHeader
            }
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if let Some(qualifier) = qualifier_in_paren_list(root, offset) {
        let syn = match qualifier {
            Qualifier::InParenList(InParenList {
                kind: ParenListKind::ParamValueAssignment | ParenListKind::PortConnections,
            }) => SynContext::Instantiation,
            Qualifier::InParenList(InParenList { kind: ParenListKind::ParameterPortList }) => {
                SynContext::ModuleHeader
            }
            Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }) => {
                SynContext::ModuleItem
            }
            _ => unreachable!(),
        };
        return (syn, Some(qualifier));
    }

    if is_in_sensitivity_list(root, offset) {
        return (SynContext::SensitivityList, None);
    }

    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return (SynContext::TopLevel, None);
    };

    if SyntaxAncestors::start_from(node).any(|n| n.kind() == syntax::SyntaxKind::MODULE_HEADER) {
        return (SynContext::ModuleHeader, None);
    }

    if SyntaxAncestors::start_from(node).any(|n| n.kind() == syntax::SyntaxKind::MODULE_DECLARATION)
    {
        return (SynContext::ModuleItem, None);
    }

    (SynContext::TopLevel, None)
}

fn qualifier_after_dot(root: SyntaxNode<'_>, offset: TextSize) -> Option<Qualifier> {
    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return None;
    };

    for anc in SyntaxAncestors::start_from(node) {
        if let Some(named) = ast::NamedPortConnection::cast(anc) {
            let (Some(dot), Some(name)) = (named.dot(), named.name()) else {
                return None;
            };

            let Some(dot_range) = dot.text_range() else {
                return None;
            };
            let Some(name_range) = name.text_range() else {
                return None;
            };

            let zone_end = named
                .open_paren()
                .and_then(|t| t.text_range())
                .map(|r| r.start())
                .unwrap_or_else(|| name_range.end());

            if offset >= dot_range.end() && offset <= zone_end && offset <= name_range.end() {
                return Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort }));
            }
        }

        if let Some(named) = ast::NamedParamAssignment::cast(anc) {
            let (Some(dot), Some(name)) = (named.dot(), named.name()) else {
                return None;
            };

            let Some(dot_range) = dot.text_range() else {
                return None;
            };
            let Some(name_range) = name.text_range() else {
                return None;
            };

            let zone_end = named
                .open_paren()
                .and_then(|t| t.text_range())
                .map(|r| r.start())
                .unwrap_or_else(|| name_range.end());

            if offset >= dot_range.end() && offset <= zone_end && offset <= name_range.end() {
                return Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedParam }));
            }
        }
    }

    let prev = token_before_offset(root, offset)?;
    (prev.kind() == syntax::Token![.])
        .then_some(Qualifier::AfterDot(AfterDot { kind: DotKind::Member }))
}

fn qualifier_after_hash(root: SyntaxNode<'_>, offset: TextSize) -> Option<Qualifier> {
    let prev = token_before_offset(root, offset)?;
    if prev.kind() != syntax::Token![#] {
        return None;
    }

    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return None;
    };

    for anc in SyntaxAncestors::start_from(node) {
        if let Some(params) = ast::ParameterValueAssignment::cast(anc) {
            let Some(hash) = params.hash() else {
                continue;
            };
            let Some(hash_range) = hash.text_range() else {
                continue;
            };
            if hash_range.end() == offset {
                return Some(Qualifier::AfterHash(AfterHash {
                    kind: HashKind::ParamValueAssignment,
                }));
            }
        }

        if let Some(params) = ast::ParameterPortList::cast(anc) {
            let Some(hash) = params.hash() else {
                continue;
            };
            let Some(hash_range) = hash.text_range() else {
                continue;
            };
            if hash_range.end() == offset {
                return Some(Qualifier::AfterHash(AfterHash {
                    kind: HashKind::ParameterPortList,
                }));
            }
        }
    }

    None
}

fn qualifier_in_paren_list(root: SyntaxNode<'_>, offset: TextSize) -> Option<Qualifier> {
    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return None;
    };

    for anc in SyntaxAncestors::start_from(node) {
        if let Some(list) = ast::ParameterValueAssignment::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::ParamValueAssignment,
                }));
            }
        }

        if let Some(list) = ast::ParameterPortList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::ParameterPortList,
                }));
            }
        }

        if let Some(list) = ast::HierarchicalInstance::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList {
                    kind: ParenListKind::PortConnections,
                }));
            }
        }

        if let Some(list) = ast::ArgumentList::cast(anc) {
            if in_parens(offset, list.open_paren(), list.close_paren()) {
                return Some(Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }));
            }
        }
    }

    None
}

fn in_parens(
    offset: TextSize,
    open_paren: Option<syntax::SyntaxToken<'_>>,
    close_paren: Option<syntax::SyntaxToken<'_>>,
) -> bool {
    let (Some(open), Some(close)) = (open_paren, close_paren) else {
        return false;
    };
    let (Some(open_range), Some(close_range)) = (open.text_range(), close.text_range()) else {
        return false;
    };
    offset >= open_range.end() && offset <= close_range.start()
}

fn token_before_offset(
    root: SyntaxNode<'_>,
    offset: TextSize,
) -> Option<SyntaxTokenWithParent<'_>> {
    let mut cursor = root.walk();
    if !cursor.goto_last_tok_before(offset) {
        return None;
    }
    cursor.to_tok_with_parent()
}

fn is_in_sensitivity_list(root: SyntaxNode<'_>, offset: TextSize) -> bool {
    let elem = root.covering_element(TextRange::empty(offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return false;
    };

    SyntaxAncestors::start_from(node).any(|n| {
        matches!(
            n.kind(),
            syntax::SyntaxKind::EVENT_CONTROL
                | syntax::SyntaxKind::EVENT_CONTROL_WITH_EXPRESSION
                | syntax::SyntaxKind::IMPLICIT_EVENT_CONTROL
                | syntax::SyntaxKind::REPEATED_EVENT_CONTROL
        )
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    };

    use syntax::SyntaxTree;

    use super::*;

    static PARSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static NEXT_FILE_ID: AtomicUsize = AtomicUsize::new(0);

    fn ctx(text: &str) -> CompletionContext {
        let _guard = PARSE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let marker = "/*caret*/";
        let off = text.find(marker).expect("missing /*caret*/");
        let mut owned = text.to_string();
        owned = owned.replace(marker, "");
        let id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = format!("test_{id}.v");
        let tree = SyntaxTree::from_text(&owned, "test", &path);
        let root = tree.root().unwrap();
        detect_completion_context(root, TextSize::from(off as u32))
    }

    #[test]
    fn detects_line_comment() {
        let c = ctx("module m; // hello /*caret*/world\nendmodule\n");
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn detects_block_comment() {
        let c = ctx("module m; /* hello /*caret*/world */ endmodule\n");
        assert_eq!(c.lex, LexContext::BlockComment);
    }

    #[test]
    fn detects_string_literal() {
        let c = ctx("module m; initial $display(\"he/*caret*/llo\"); endmodule\n");
        assert_eq!(c.lex, LexContext::StringLiteral);
    }

    #[test]
    fn detects_named_port_after_dot() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(./*caret*/a()); endmodule\n");
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort })));
    }

    #[test]
    fn detects_named_param_after_dot() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(./*caret*/W(1)) u0(); endmodule\n",
        );
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedParam })));
    }

    #[test]
    fn detects_hier_member_after_dot() {
        let c = ctx("module m; wire a; initial top.sub./*caret*/a; endmodule\n");
        assert_eq!(c.syn, SynContext::HierRef);
        assert_eq!(c.qualifier, Some(Qualifier::AfterDot(AfterDot { kind: DotKind::Member })));
    }

    #[test]
    fn detects_param_value_assignment_list() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(/*caret*/1) u0(); endmodule\n",
        );
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InParenList(InParenList { kind: ParenListKind::ParamValueAssignment }))
        );
    }

    #[test]
    fn detects_parameter_port_list() {
        let c = ctx("module m #(/*caret*/parameter W=1) (); endmodule\n");
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InParenList(InParenList { kind: ParenListKind::ParameterPortList }))
        );
    }

    #[test]
    fn detects_port_connection_list() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(/*caret*/); endmodule\n");
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InParenList(InParenList { kind: ParenListKind::PortConnections }))
        );
    }

    #[test]
    fn detects_argument_list() {
        let c = ctx("module m; initial f(/*caret*/); endmodule\n");
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }))
        );
    }
}
