//! Completion context detection.

mod caret;
mod decl_name;
mod lex;
mod syn;
mod util;

use base_db::source_db::SourceDb;
use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{SyntaxNode, ast::AstNode};
use utils::line_index::{TextRange, TextSize};

use self::caret::CaretSnapshot;

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
pub enum PortListKind {
    Ansi,
    NonAnsi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtKind {
    EventControl,
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
pub struct InPortList {
    pub kind: PortListKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Qualifier {
    AfterDot(AfterDot),
    AfterHash(AfterHash),
    InParenList(InParenList),
    InPortList(InPortList),
    AfterAt(AtKind),
    AfterBacktick,
    InNamedPortConnExpr,
    InNamedParamAssignExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerChar {
    Dot,
    OpenParen,
    Comma,
    At,
    Hash,
    Backtick,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub replacement: TextRange,
    pub prefix: String,
    pub trigger: Option<TriggerChar>,
    pub lex: LexContext,
    pub syn: SynContext,
    pub qualifier: Option<Qualifier>,
    pub in_decl_name: bool,
}

pub(crate) fn completion_context(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    let sema = Semantics::new(db);
    let file = sema.parse(file_id);
    let expected_ident_offsets = db.expected_identifier_offsets(file_id);
    detect_completion_context_impl(file.syntax(), offset, trigger, Some(&expected_ident_offsets))
}

pub fn detect_completion_context(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    detect_completion_context_impl(root, offset, trigger, None)
}

pub fn detect_completion_context_with_expected_identifier_offsets(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    expected_identifier_offsets: &[TextSize],
) -> CompletionContext {
    detect_completion_context_impl(root, offset, trigger, Some(expected_identifier_offsets))
}

fn detect_completion_context_impl(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    expected_identifier_offsets: Option<&[TextSize]>,
) -> CompletionContext {
    let caret = CaretSnapshot::new(root, offset);
    let (replacement, prefix) = caret.replacement_and_prefix();

    let lex = lex::detect_lex_context(&caret);
    if lex != LexContext::Code {
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            syn: SynContext::TopLevel,
            qualifier: None,
            in_decl_name: false,
        };
    }

    let in_decl_name = decl_name::is_in_decl_name(&caret, expected_identifier_offsets);
    let (syn, qualifier) = syn::detect_syn_context(&caret, trigger);
    CompletionContext { replacement, prefix, trigger, lex, syn, qualifier, in_decl_name }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
    };

    use syntax::{Compilation, SyntaxTree};

    use super::*;

    static PARSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static NEXT_FILE_ID: AtomicUsize = AtomicUsize::new(0);

    fn ctx(text: &str) -> CompletionContext {
        ctx_with_trigger(text, None)
    }

    fn ctx_with_trigger(text: &str, trigger: Option<TriggerChar>) -> CompletionContext {
        let _guard = PARSE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let marker = "/*caret*/";
        let off = text.find(marker).expect("missing /*caret*/");
        let mut owned = text.to_string();
        owned = owned.replace(marker, "");
        let id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = format!("test_{id}.v");
        let tree = SyntaxTree::from_text(&owned, "test", &path);

        let mut compilation = Compilation::new();
        compilation.add_syntax_tree(tree.clone());
        let mut expected_ident_offsets: Vec<TextSize> = compilation
            .parse_diag_offsets_by_name("ExpectedIdentifier")
            .into_iter()
            .filter_map(|offset| u32::try_from(offset).ok().map(TextSize::from))
            .collect();
        expected_ident_offsets.sort();
        expected_ident_offsets.dedup();

        let root = tree.root().unwrap();
        detect_completion_context_with_expected_identifier_offsets(
            root,
            TextSize::from(off as u32),
            trigger,
            &expected_ident_offsets,
        )
    }

    #[test]
    fn detects_line_comment() {
        let c = ctx("module m; // hello /*caret*/world\nendmodule\n");
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn detects_line_comment_at_file_start() {
        // regression: line comment at file start should be detected
        let c = ctx("// hello /*caret*/world\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn detects_line_comment_at_file_start_with_comma() {
        // regression: comma trigger in line comment at file start
        let c = ctx_with_trigger("// ,/*caret*/,\nmodule m; endmodule\n", Some(TriggerChar::Comma));
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn detects_line_comment_in_middle_of_file() {
        // regression: line comment in middle of file (before any module)
        let c = ctx("// line1\n// line2 /*caret*/\nmodule m; endmodule\n");
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
    fn detects_preproc_directive() {
        let c = ctx("`define /*caret*/FOO 1\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::PreprocDirective);
    }

    #[test]
    fn detects_line_comment_at_eof_top_level() {
        let c = ctx("// ,/*caret*/");
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn detects_line_comment_at_eol_boundary_top_level() {
        let c = ctx("// ,/*caret*/\n");
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn detects_line_comment_before_directive() {
        // regression: line comment before `timescale should still be detected
        let c = ctx("// comment/*caret*/\n`timescale 1ns / 1ps\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::LineComment);
    }

    #[test]
    fn replacement_includes_keywords() {
        let c = ctx("module/*caret*/ m; endmodule\n");
        assert_eq!(c.prefix, "module");
        assert!(!c.replacement.is_empty());
    }

    #[test]
    fn replacement_at_eof_identifier() {
        let c = ctx("mo/*caret*/");
        assert_eq!(c.prefix, "mo");
        assert!(!c.replacement.is_empty());
    }

    #[test]
    fn detects_named_port_after_dot() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(./*caret*/a()); endmodule\n");
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedPort })));
    }

    #[test]
    fn detects_named_port_after_dot_without_name() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(./*caret*/); endmodule\n");
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
    fn detects_named_param_after_dot_without_name() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(./*caret*/) u0(); endmodule\n",
        );
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::AfterDot(AfterDot { kind: DotKind::NamedParam })));
    }

    #[test]
    fn detects_named_port_conn_expr_after_name() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(.a(/*caret*/)); endmodule\n");
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::InNamedPortConnExpr));
    }

    #[test]
    fn detects_named_port_conn_expr_after_name_without_close_paren() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(.a(/*caret*/\nendmodule\n");
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::InNamedPortConnExpr));
    }

    #[test]
    fn detects_named_param_assign_expr_after_name() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(.W(/*caret*/)) u0(); endmodule\n",
        );
        assert_eq!(c.syn, SynContext::Instantiation);
        assert_eq!(c.qualifier, Some(Qualifier::InNamedParamAssignExpr));
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
    fn detects_port_connection_list_without_close_paren() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(/*caret*/\nendmodule\n");
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InParenList(InParenList { kind: ParenListKind::PortConnections }))
        );
    }

    #[test]
    fn detects_port_connection_list_trigger_open_paren_without_close_paren() {
        let c = ctx_with_trigger(
            "module m(input a); endmodule\nmodule top; m u0(/*caret*/\nendmodule\n",
            Some(TriggerChar::OpenParen),
        );
        assert_eq!(c.syn, SynContext::Instantiation);
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

    #[test]
    fn detects_argument_list_trigger_open_paren_without_close_paren() {
        let c = ctx_with_trigger(
            "module m; initial f(/*caret*/\nendmodule\n",
            Some(TriggerChar::OpenParen),
        );
        assert_eq!(c.syn, SynContext::ModuleItem);
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InParenList(InParenList { kind: ParenListKind::Arguments }))
        );
    }

    #[test]
    fn detects_ansi_port_list() {
        let c = ctx("module m(input /*caret*/a); endmodule\n");
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InPortList(InPortList { kind: PortListKind::Ansi }))
        );
    }

    #[test]
    fn detects_ansi_port_list_trigger_comma() {
        let c = ctx_with_trigger(
            "module m(input a, /*caret*/b); endmodule\n",
            Some(TriggerChar::Comma),
        );
        assert_eq!(c.syn, SynContext::ModuleHeader);
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InPortList(InPortList { kind: PortListKind::Ansi }))
        );
    }

    #[test]
    fn detects_non_ansi_port_list() {
        let c = ctx("module m(a, /*caret*/b); input a; output b; endmodule\n");
        assert_eq!(
            c.qualifier,
            Some(Qualifier::InPortList(InPortList { kind: PortListKind::NonAnsi }))
        );
    }

    #[test]
    fn detects_after_at_trigger() {
        let c = ctx_with_trigger(
            "module m; always @/*caret*/(posedge clk) begin end endmodule\n",
            Some(TriggerChar::At),
        );
        assert_eq!(c.syn, SynContext::SensitivityList);
        assert_eq!(c.qualifier, Some(Qualifier::AfterAt(AtKind::EventControl)));
    }

    #[test]
    fn detects_after_backtick_trigger() {
        let c = ctx_with_trigger(
            "module m; initial `/*caret*/FOO; endmodule\n",
            Some(TriggerChar::Backtick),
        );
        assert_eq!(c.qualifier, Some(Qualifier::AfterBacktick));
    }

    #[test]
    fn detects_decl_name_in_ansi_port_list() {
        let c = ctx("module m(input [3:0] /*caret*/); endmodule\n");
        assert!(c.in_decl_name);
    }

    #[test]
    fn detects_decl_name_in_tf_port_list() {
        let c = ctx("module m; task t(input [3:0] /*caret*/); endtask endmodule\n");
        assert!(c.in_decl_name);
    }

    #[test]
    fn detects_decl_name_in_ansi_port_list_multiline() {
        let c = ctx("module m(\n  input [3:0] /*caret*/\n);\nendmodule\n");
        assert!(c.in_decl_name);
    }

    #[test]
    fn detects_decl_name_in_tf_port_list_multiline() {
        let c = ctx("module m;\ntask t(\n  input [3:0] /*caret*/\n);\nendtask\nendmodule\n");
        assert!(c.in_decl_name);
    }
}
