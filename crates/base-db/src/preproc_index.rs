use smol_str::{SmolStr, ToSmolStr};
use syntax::{
    PreprocessorDirective, PreprocessorDirectiveToken, PreprocessorMacroParam, SyntaxKind,
    SyntaxTree, SyntaxTreeOptions,
};
use utils::line_index::{TextRange, TextSize};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocFileIndex {
    pub directives: Vec<MacroDirective>,
    pub defines: Vec<MacroDefine>,
    pub undefs: Vec<MacroUndef>,
    pub includes: Vec<MacroInclude>,
    pub conditionals: Vec<MacroConditional>,
    pub usages: Vec<MacroUsage>,
    pub inactive_ranges: Vec<TextRange>,
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

pub fn preproc_file_index_from_text(text: &str, options: &SyntaxTreeOptions) -> PreprocFileIndex {
    let mut index = PreprocFileIndex::default();
    for directive in SyntaxTree::preprocessor_directives(text, "source", "", options) {
        collect_preprocessor_directive(&mut index, directive);
    }
    index
}

pub fn literal_include_directives(text: &str) -> Vec<MacroInclude> {
    preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion())
        .includes
        .into_iter()
        .filter(|include| matches!(include.target, MacroIncludeTarget::Literal { .. }))
        .collect()
}

fn range_to_text_range(range: std::ops::Range<usize>) -> Option<TextRange> {
    Some(TextRange::new(
        TextSize::from(u32::try_from(range.start).ok()?),
        TextSize::from(u32::try_from(range.end).ok()?),
    ))
}

fn collect_preprocessor_directive(index: &mut PreprocFileIndex, directive: PreprocessorDirective) {
    index.inactive_ranges.extend(directive.disabled_ranges.iter().filter_map(|range| {
        let range = range_to_text_range(range.clone())?;
        (!range.is_empty()).then_some(range)
    }));

    let kind = directive.kind;
    match kind {
        SyntaxKind::DEFINE_DIRECTIVE => {
            let directive_index = index.defines.len();
            let define = collect_preprocessor_define(directive);
            let range = define.range;
            index.defines.push(define);
            push_preprocessor_directive(index, MacroDirectiveKind::Define, directive_index, range);
        }
        SyntaxKind::UNDEF_DIRECTIVE => {
            let directive_index = index.undefs.len();
            let range = directive.range.and_then(range_to_text_range);
            index.undefs.push(MacroUndef {
                name: directive.name.as_ref().map(preprocessor_token_value),
                range,
            });
            push_preprocessor_directive(index, MacroDirectiveKind::Undef, directive_index, range);
        }
        SyntaxKind::INCLUDE_DIRECTIVE => {
            let directive_index = index.includes.len();
            let range = directive.range.and_then(range_to_text_range);
            let target = directive
                .include_file_name
                .map(|token| include_target_from_raw(token.raw_text.to_smolstr()))
                .unwrap_or_else(|| MacroIncludeTarget::Token { raw: SmolStr::new("") });
            index.includes.push(MacroInclude { target, range });
            push_preprocessor_directive(index, MacroDirectiveKind::Include, directive_index, range);
        }
        SyntaxKind::IF_DEF_DIRECTIVE
        | SyntaxKind::IF_N_DEF_DIRECTIVE
        | SyntaxKind::ELS_IF_DIRECTIVE => {
            let directive_index = index.conditionals.len();
            let range = directive.range.and_then(range_to_text_range);
            index.conditionals.push(MacroConditional {
                kind: preprocessor_conditional_kind(kind),
                expr: directive
                    .expr_tokens
                    .into_iter()
                    .map(macro_token_from_preprocessor)
                    .collect(),
                range,
            });
            push_preprocessor_directive(
                index,
                MacroDirectiveKind::Conditional,
                directive_index,
                range,
            );
        }
        SyntaxKind::ELSE_DIRECTIVE | SyntaxKind::END_IF_DIRECTIVE => {
            let directive_index = index.conditionals.len();
            let range = directive.range.and_then(range_to_text_range);
            index.conditionals.push(MacroConditional {
                kind: preprocessor_conditional_kind(kind),
                expr: Vec::new(),
                range,
            });
            push_preprocessor_directive(index, MacroDirectiveKind::Branch, directive_index, range);
        }
        SyntaxKind::MACRO_USAGE => {
            let directive_index = index.usages.len();
            let range = directive.range.and_then(range_to_text_range);
            index.usages.push(MacroUsage {
                name: directive.name.map(|token| macro_name(token.value_text)),
                range,
            });
            push_preprocessor_directive(index, MacroDirectiveKind::Usage, directive_index, range);
        }
        _ => {}
    }
}

fn collect_preprocessor_define(directive: PreprocessorDirective) -> MacroDefine {
    MacroDefine {
        name: directive.name.as_ref().map(preprocessor_token_value),
        params: (!directive.params.is_empty())
            .then(|| directive.params.into_iter().map(macro_param_from_preprocessor).collect()),
        body: directive.body_tokens.into_iter().map(macro_token_from_preprocessor).collect(),
        range: directive.range.and_then(range_to_text_range),
    }
}

fn macro_param_from_preprocessor(param: PreprocessorMacroParam) -> MacroParam {
    MacroParam {
        name: param.name.as_ref().map(preprocessor_token_value),
        default: param
            .default_tokens
            .map(|tokens| tokens.into_iter().map(macro_token_from_preprocessor).collect()),
        range: param.range.and_then(range_to_text_range),
    }
}

fn macro_token_from_preprocessor(token: PreprocessorDirectiveToken) -> MacroToken {
    MacroToken {
        raw: token.raw_text.to_smolstr(),
        value: token.value_text.to_smolstr(),
        range: token.range.and_then(range_to_text_range),
    }
}

fn preprocessor_token_value(token: &PreprocessorDirectiveToken) -> SmolStr {
    token.value_text.to_smolstr()
}

fn preprocessor_conditional_kind(kind: SyntaxKind) -> MacroConditionalKind {
    match kind {
        SyntaxKind::IF_DEF_DIRECTIVE => MacroConditionalKind::IfDef,
        SyntaxKind::IF_N_DEF_DIRECTIVE => MacroConditionalKind::IfNDef,
        SyntaxKind::ELS_IF_DIRECTIVE => MacroConditionalKind::ElsIf,
        SyntaxKind::ELSE_DIRECTIVE => MacroConditionalKind::Else,
        SyntaxKind::END_IF_DIRECTIVE => MacroConditionalKind::EndIf,
        _ => unreachable!(),
    }
}

fn push_preprocessor_directive(
    index: &mut PreprocFileIndex,
    kind: MacroDirectiveKind,
    directive_index: usize,
    range: Option<TextRange>,
) {
    index.directives.push(MacroDirective { kind, range, index: directive_index });
}

fn include_target_from_raw(raw: SmolStr) -> MacroIncludeTarget {
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

fn macro_name(name: String) -> SmolStr {
    name.strip_prefix('`').unwrap_or(&name).to_smolstr()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(text: &str) -> PreprocFileIndex {
        preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion())
    }

    fn literal_includes(text: &str) -> Vec<MacroInclude> {
        literal_include_directives(text)
    }

    fn index_with_predefines(text: &str, predefines: Vec<String>) -> PreprocFileIndex {
        preproc_file_index_from_text(
            text,
            &SyntaxTreeOptions { predefines, ..SyntaxTreeOptions::without_include_expansion() },
        )
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

    #[test]
    fn scans_literal_include_directives_without_full_parse() {
        let includes = literal_includes(
            r#"`include "defs.svh"
`include <vendor/pkg.svh>
`include SOME_MACRO
"`include \"string.svh\""
// `include "comment.svh"
/* `include "block_comment.svh" */
"#,
        );

        assert_eq!(
            includes.iter().map(|include| &include.target).collect::<Vec<_>>(),
            vec![
                &MacroIncludeTarget::Literal {
                    path: SmolStr::new("defs.svh"),
                    raw: SmolStr::new("\"defs.svh\"")
                },
                &MacroIncludeTarget::Literal {
                    path: SmolStr::new("vendor/pkg.svh"),
                    raw: SmolStr::new("<vendor/pkg.svh>")
                },
            ]
        );
    }

    #[test]
    fn does_not_scan_include_target_past_line_end() {
        let includes = literal_includes(
            r#"`include
"next_line.svh"
`include "same_line.svh"
"#,
        );

        assert_eq!(includes.len(), 1);
        assert_eq!(
            includes[0].target,
            MacroIncludeTarget::Literal {
                path: SmolStr::new("same_line.svh"),
                raw: SmolStr::new("\"same_line.svh\"")
            }
        );
    }

    #[test]
    fn preprocessor_index_honors_predefined_conditional_includes() {
        let text = r#"`ifdef USE_A
`include "a.svh"
`else
`include "b.svh"
`endif
"#;

        let without_define = index_with_predefines(text, Vec::new());
        let with_define = index_with_predefines(text, vec!["USE_A=1".to_owned()]);

        assert_eq!(
            without_define.includes[0].target,
            MacroIncludeTarget::Literal {
                path: SmolStr::new("b.svh"),
                raw: SmolStr::new("\"b.svh\"")
            }
        );
        assert_eq!(
            with_define.includes[0].target,
            MacroIncludeTarget::Literal {
                path: SmolStr::new("a.svh"),
                raw: SmolStr::new("\"a.svh\"")
            }
        );
    }

    #[test]
    fn records_inactive_preprocessor_branch_ranges() {
        let text = r#"`ifdef USE_A
logic active;
`else
logic inactive;
`endif
"#;

        let without_define = index_with_predefines(text, Vec::new());
        let with_define = index_with_predefines(text, vec!["USE_A=1".to_owned()]);

        let inactive_range = without_define.inactive_ranges[0];
        assert_eq!(
            &text[usize::from(inactive_range.start())..usize::from(inactive_range.end())],
            "logic active;"
        );

        let inactive_range = with_define.inactive_ranges[0];
        assert_eq!(
            &text[usize::from(inactive_range.start())..usize::from(inactive_range.end())],
            "logic inactive;"
        );
    }
}
