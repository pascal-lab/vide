use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    SyntaxElement, SyntaxNode, SyntaxToken, SyntaxTree, Trivia, WalkEvent,
    ast::{self, AstNode},
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::line_index::TextRange;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocFileIndex {
    pub directives: Vec<MacroDirective>,
    pub defines: Vec<MacroDefine>,
    pub undefs: Vec<MacroUndef>,
    pub includes: Vec<MacroInclude>,
    pub conditionals: Vec<MacroConditional>,
    pub usages: Vec<MacroUsage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroDirectiveKind {
    Define,
    Undef,
    Include,
    Conditional,
    Branch,
    Usage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDirective {
    pub kind: MacroDirectiveKind,
    pub range: Option<TextRange>,
    pub index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroDefine {
    pub name: Option<SmolStr>,
    pub params: Option<Vec<MacroParam>>,
    pub body: Vec<MacroToken>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroParam {
    pub name: Option<SmolStr>,
    pub default: Option<Vec<MacroToken>>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUndef {
    pub name: Option<SmolStr>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroInclude {
    pub target: MacroIncludeTarget,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroIncludeTarget {
    Literal { path: SmolStr, raw: SmolStr },
    Token { raw: SmolStr },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroConditionalKind {
    IfDef,
    IfNDef,
    ElsIf,
    Else,
    EndIf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroConditional {
    pub kind: MacroConditionalKind,
    pub expr: Vec<MacroToken>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroUsage {
    pub name: Option<SmolStr>,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroToken {
    pub raw: SmolStr,
    pub value: SmolStr,
    pub range: Option<TextRange>,
}

pub fn preproc_file_index(tree: &SyntaxTree) -> PreprocFileIndex {
    let Some(root) = tree.root() else {
        return PreprocFileIndex::default();
    };

    let mut index = PreprocFileIndex::default();
    for node in directive_nodes(root) {
        collect_directive(&mut index, node);
    }
    index
}

fn directive_nodes(root: SyntaxNode<'_>) -> Vec<SyntaxNode<'_>> {
    let mut nodes = Vec::new();
    for event in root.elem_preorder() {
        let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
            continue;
        };

        for trivia in token.tok.trivias() {
            if trivia.kind() != Trivia!["`"] {
                continue;
            }

            let Some(node) = trivia.syntax() else {
                continue;
            };
            if ast::Directive::cast(node).is_some() {
                nodes.push(node);
            }
        }
    }
    nodes
}

fn collect_directive(index: &mut PreprocFileIndex, node: SyntaxNode<'_>) {
    let Some(directive) = ast::Directive::cast(node) else {
        return;
    };

    match directive {
        ast::Directive::DefineDirective(directive) => {
            let directive_index = index.defines.len();
            index.defines.push(collect_define(directive));
            push_directive(index, MacroDirectiveKind::Define, node, directive_index);
        }
        ast::Directive::UndefDirective(directive) => {
            let directive_index = index.undefs.len();
            index.undefs.push(collect_undef(directive));
            push_directive(index, MacroDirectiveKind::Undef, node, directive_index);
        }
        ast::Directive::IncludeDirective(directive) => {
            let directive_index = index.includes.len();
            index.includes.push(collect_include(directive));
            push_directive(index, MacroDirectiveKind::Include, node, directive_index);
        }
        ast::Directive::ConditionalBranchDirective(directive) => {
            let directive_index = index.conditionals.len();
            index.conditionals.push(collect_conditional_branch(directive));
            push_directive(index, MacroDirectiveKind::Conditional, node, directive_index);
        }
        ast::Directive::UnconditionalBranchDirective(directive) => {
            let directive_index = index.conditionals.len();
            index.conditionals.push(collect_unconditional_branch(directive));
            push_directive(index, MacroDirectiveKind::Branch, node, directive_index);
        }
        ast::Directive::MacroUsage(directive) => {
            let directive_index = index.usages.len();
            index.usages.push(collect_usage(directive));
            push_directive(index, MacroDirectiveKind::Usage, node, directive_index);
        }
        _ => {}
    }
}

fn push_directive(
    index: &mut PreprocFileIndex,
    kind: MacroDirectiveKind,
    node: SyntaxNode<'_>,
    directive_index: usize,
) {
    index.directives.push(MacroDirective {
        kind,
        range: node.text_range(),
        index: directive_index,
    });
}

fn collect_define(directive: ast::DefineDirective<'_>) -> MacroDefine {
    let params = directive.formal_arguments().map(|args| {
        args.args()
            .children()
            .map(|arg| MacroParam {
                name: token_value(arg.name()),
                default: arg.default_value().map(|default| collect_token_list(default.tokens())),
                range: arg.syntax().text_range(),
            })
            .collect()
    });

    MacroDefine {
        name: token_value(directive.name()),
        params,
        body: collect_token_list(directive.body()),
        range: directive.syntax().text_range(),
    }
}

fn collect_undef(directive: ast::UndefDirective<'_>) -> MacroUndef {
    MacroUndef { name: token_value(directive.name()), range: directive.syntax().text_range() }
}

fn collect_include(directive: ast::IncludeDirective<'_>) -> MacroInclude {
    let target = directive
        .file_name()
        .map(include_target)
        .unwrap_or_else(|| MacroIncludeTarget::Token { raw: SmolStr::new("") });
    MacroInclude { target, range: directive.syntax().text_range() }
}

fn include_target(token: SyntaxToken<'_>) -> MacroIncludeTarget {
    let raw = token.raw_text().to_string().to_smolstr();
    if let Some(path) = strip_include_delimiters(&raw) {
        MacroIncludeTarget::Literal { path: path.to_smolstr(), raw }
    } else {
        MacroIncludeTarget::Token { raw }
    }
}

fn strip_include_delimiters(raw: &str) -> Option<&str> {
    let bytes = raw.as_bytes();
    let (first, last) = (*bytes.first()?, *bytes.last()?);
    match (first, last) {
        (b'"', b'"') | (b'<', b'>') if raw.len() >= 2 => Some(&raw[1..raw.len() - 1]),
        _ => None,
    }
}

fn collect_conditional_branch(directive: ast::ConditionalBranchDirective<'_>) -> MacroConditional {
    let kind = if directive.as_if_def_directive().is_some() {
        MacroConditionalKind::IfDef
    } else if directive.as_if_n_def_directive().is_some() {
        MacroConditionalKind::IfNDef
    } else {
        MacroConditionalKind::ElsIf
    };
    MacroConditional {
        kind,
        expr: collect_node_tokens(directive.expr().syntax()),
        range: directive.syntax().text_range(),
    }
}

fn collect_unconditional_branch(
    directive: ast::UnconditionalBranchDirective<'_>,
) -> MacroConditional {
    let kind = if directive.as_else_directive().is_some() {
        MacroConditionalKind::Else
    } else {
        MacroConditionalKind::EndIf
    };
    MacroConditional { kind, expr: Vec::new(), range: directive.syntax().text_range() }
}

fn collect_usage(directive: ast::MacroUsage<'_>) -> MacroUsage {
    MacroUsage {
        name: directive.directive().map(|token| macro_name(token.value_text().to_string())),
        range: directive.syntax().text_range(),
    }
}

fn collect_token_list(list: ast::TokenList<'_>) -> Vec<MacroToken> {
    let context = list.syntax();
    list.children().map(|token| macro_token(token, context)).collect()
}

fn collect_node_tokens(node: SyntaxNode<'_>) -> Vec<MacroToken> {
    node.elem_preorder()
        .filter_map(|event| match event {
            WalkEvent::Enter(SyntaxElement::Token(token)) => Some(macro_token(token.tok, node)),
            _ => None,
        })
        .collect()
}

fn macro_token(token: SyntaxToken<'_>, context: SyntaxNode<'_>) -> MacroToken {
    MacroToken {
        raw: token.raw_text().to_string().to_smolstr(),
        value: token.value_text().to_string().to_smolstr(),
        range: token.text_range_in(context),
    }
}

fn token_value(token: Option<SyntaxToken<'_>>) -> Option<SmolStr> {
    token.map(|token| token.value_text().to_string().to_smolstr())
}

fn macro_name(name: String) -> SmolStr {
    name.strip_prefix('`').unwrap_or(&name).to_smolstr()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(text: &str) -> PreprocFileIndex {
        let tree = SyntaxTree::from_text(text, "source", "");
        preproc_file_index(&tree)
    }

    #[test]
    fn indexes_define_include_undef_and_usage_directives() {
        let index = index(
            r#"`define WIDTH(W=8) logic [W-1:0]
`include "defs.svh"
`undef WIDTH
`WIDTH
module top;
endmodule
"#,
        );

        assert_eq!(index.defines.len(), 1);
        assert_eq!(index.defines[0].name.as_deref(), Some("WIDTH"));
        assert_eq!(index.defines[0].params.as_ref().unwrap()[0].name.as_deref(), Some("W"));
        assert_eq!(
            index.defines[0].params.as_ref().unwrap()[0].default.as_ref().unwrap()[0].raw.as_str(),
            "8"
        );
        assert!(index.defines[0].body.iter().any(|token| token.value == "logic"));

        assert_eq!(index.includes.len(), 1);
        assert_eq!(
            index.includes[0].target,
            MacroIncludeTarget::Literal {
                path: SmolStr::new("defs.svh"),
                raw: SmolStr::new("\"defs.svh\"")
            }
        );

        assert_eq!(index.undefs[0].name.as_deref(), Some("WIDTH"));
        assert_eq!(index.usages[0].name.as_deref(), Some("WIDTH"));
        assert_eq!(
            index.directives.iter().map(|directive| directive.kind).collect::<Vec<_>>(),
            vec![
                MacroDirectiveKind::Define,
                MacroDirectiveKind::Include,
                MacroDirectiveKind::Undef,
                MacroDirectiveKind::Usage,
            ]
        );
    }

    #[test]
    fn indexes_conditional_directive_nodes() {
        let index = index(
            r#"`ifdef USE_A
`include "a.sv"
`else
`include "b.sv"
`endif
"#,
        );

        assert_eq!(
            index.conditionals.iter().map(|conditional| conditional.kind).collect::<Vec<_>>(),
            vec![
                MacroConditionalKind::IfDef,
                MacroConditionalKind::Else,
                MacroConditionalKind::EndIf,
            ]
        );
        assert_eq!(index.conditionals[0].expr[0].value.as_str(), "USE_A");
    }
}
