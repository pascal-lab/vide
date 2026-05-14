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
use syntax::SyntaxNode;
use utils::line_index::{TextRange, TextSize};

use self::caret::CaretSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexContext {
    Code,
    LineComment,
    BlockComment,
    Literal,
    PreprocDirective,
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
pub enum TriggerChar {
    Dot,
    OpenParen,
    Comma,
    At,
    Hash,
    Backtick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSite {
    Forbidden,
    PreprocDirective,
    TopLevel,
    ModuleHeader,
    ModuleItemStart,
    Expr,
    NamedPortName,
    NamedParamName,
    MemberAccess,
    NamedPortConnExpr,
    NamedParamAssignExpr,
    AfterParamValueAssignmentHash,
    AfterParameterPortListHash,
    ParamValueAssignment,
    ParameterPortList,
    PortConnections,
    Arguments,
    AnsiPortList,
    NonAnsiPortList,
    AfterAtEventControl,
    SensitivityList,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub replacement: TextRange,
    pub prefix: String,
    pub trigger: Option<TriggerChar>,
    pub lex: LexContext,
    pub site: CompletionSite,
    pub in_decl_name: bool,
}

struct DirectiveWord {
    replacement: TextRange,
    prefix: String,
}

pub(crate) fn completion_context(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    let sema = Semantics::new(db);
    let root = sema.parse_root(file_id);
    let text = db.file_text(file_id);
    let expected_ident_offsets = db.expected_identifier_offsets(file_id);
    detect_completion_context_impl(
        root,
        offset,
        trigger,
        Some(&text),
        Some(&expected_ident_offsets),
    )
}

pub fn detect_completion_context(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    detect_completion_context_impl(root, offset, trigger, None, None)
}

pub fn detect_completion_context_with_expected_identifier_offsets(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    expected_identifier_offsets: &[TextSize],
) -> CompletionContext {
    detect_completion_context_impl(root, offset, trigger, None, Some(expected_identifier_offsets))
}

pub fn detect_completion_context_with_source_text(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    source_text: &str,
    expected_identifier_offsets: &[TextSize],
) -> CompletionContext {
    detect_completion_context_impl(
        root,
        offset,
        trigger,
        Some(source_text),
        Some(expected_identifier_offsets),
    )
}

fn detect_completion_context_impl(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    source_text: Option<&str>,
    expected_identifier_offsets: Option<&[TextSize]>,
) -> CompletionContext {
    let caret = CaretSnapshot::new(root, offset);
    let (mut replacement, mut prefix) = caret.replacement_and_prefix();

    let lex = lex::detect_lex_context(&caret);
    if lex != LexContext::Code {
        let site = if lex == LexContext::PreprocDirective
            && let Some(word) = directive_word_at_offset(source_text, offset)
        {
            replacement = word.replacement;
            prefix = word.prefix;
            CompletionSite::PreprocDirective
        } else {
            CompletionSite::Forbidden
        };
        return CompletionContext { replacement, prefix, trigger, lex, site, in_decl_name: false };
    }

    if let Some(word) = directive_word_at_offset(source_text, offset) {
        replacement = word.replacement;
        prefix = word.prefix;
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            site: CompletionSite::PreprocDirective,
            in_decl_name: false,
        };
    }

    let in_decl_name = decl_name::is_in_decl_name(&caret, expected_identifier_offsets);
    let mut site = syn::detect_completion_site(&caret);
    if in_decl_name {
        site = CompletionSite::Forbidden;
    }
    CompletionContext { replacement, prefix, trigger, lex, site, in_decl_name }
}

fn directive_word_at_offset(source_text: Option<&str>, offset: TextSize) -> Option<DirectiveWord> {
    let source_text = source_text?;
    let offset = usize::from(offset);
    if offset == 0 || offset > source_text.len() || !source_text.is_char_boundary(offset) {
        return None;
    }

    let bytes = source_text.as_bytes();
    let mut start = offset;
    while start > 0 && is_directive_name_byte(bytes[start - 1]) {
        start -= 1;
    }

    if start == 0 || bytes[start - 1] != b'`' {
        return None;
    }

    let mut end = offset;
    while end < bytes.len() && is_directive_name_byte(bytes[end]) {
        end += 1;
    }

    let prefix = source_text[start..offset].to_string();
    let replacement = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    Some(DirectiveWord { replacement, prefix })
}

fn is_directive_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
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
            .parse_diag_offsets_by_name("ExpectedIdentifier", &[])
            .into_iter()
            .filter_map(|offset| u32::try_from(offset).ok().map(TextSize::from))
            .collect();
        expected_ident_offsets.sort();
        expected_ident_offsets.dedup();

        let root = tree.root().unwrap();
        detect_completion_context_with_source_text(
            root,
            TextSize::from(off as u32),
            trigger,
            &owned,
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
        assert_eq!(c.lex, LexContext::Literal);
    }

    #[test]
    fn detects_literal() {
        let c = ctx("module m; initial x = 12/*caret*/34; endmodule\n");
        assert_eq!(c.lex, LexContext::Literal);
    }

    #[test]
    fn detects_based_literal() {
        let c = ctx("module m; initial x = 4'b10/*caret*/10; endmodule\n");
        assert_eq!(c.lex, LexContext::Literal);
    }

    #[test]
    fn detects_typing_based_literal_after_quote() {
        let c = ctx("module m; initial x = 4'/*caret*/; endmodule\n");
        assert_eq!(c.lex, LexContext::Literal);
    }

    #[test]
    fn detects_typing_based_literal_after_base() {
        let c = ctx("module m; initial x = 4'b/*caret*/; endmodule\n");
        assert_eq!(c.lex, LexContext::Literal);
    }

    #[test]
    fn detects_typing_based_literal_after_digits() {
        let c = ctx("module m; initial x = 4'b0001/*caret*/; endmodule\n");
        assert_eq!(c.lex, LexContext::Literal);
    }

    #[test]
    fn detects_preproc_directive() {
        let c = ctx("`define /*caret*/FOO 1\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::PreprocDirective);
        assert_eq!(c.site, CompletionSite::Forbidden);
    }

    #[test]
    fn detects_preproc_directive_at_boundary() {
        let c = ctx("`define FOO/*caret*/\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::PreprocDirective);
        assert_eq!(c.site, CompletionSite::Forbidden);
    }

    #[test]
    fn detects_preproc_directive_keyword() {
        let c = ctx("`de/*caret*/fine FOO 1\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::Code);
        assert_eq!(c.site, CompletionSite::PreprocDirective);
        assert_eq!(c.prefix, "de");
    }

    #[test]
    fn normalizes_preproc_directive_word_replacement() {
        let c = ctx("`de/*caret*/fine FOO 1\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::Code);
        assert_eq!(c.site, CompletionSite::PreprocDirective);
        assert_eq!(c.prefix, "de");
        assert_eq!(c.replacement, TextRange::new(TextSize::from(1), TextSize::from(7)));
    }

    #[test]
    fn detects_inline_preproc_directive_word() {
        let c = ctx("module m; initial `de/*caret*/; endmodule\n");
        assert_eq!(c.lex, LexContext::Code);
        assert_eq!(c.site, CompletionSite::PreprocDirective);
        assert_eq!(c.prefix, "de");
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
        assert_eq!(c.site, CompletionSite::NamedPortName);
    }

    #[test]
    fn detects_named_port_after_dot_without_name() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(./*caret*/); endmodule\n");
        assert_eq!(c.site, CompletionSite::NamedPortName);
    }

    #[test]
    fn detects_named_param_after_dot() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(./*caret*/W(1)) u0(); endmodule\n",
        );
        assert_eq!(c.site, CompletionSite::NamedParamName);
    }

    #[test]
    fn detects_named_param_after_dot_without_name() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(./*caret*/) u0(); endmodule\n",
        );
        assert_eq!(c.site, CompletionSite::NamedParamName);
    }

    #[test]
    fn detects_named_port_conn_expr_after_name() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(.a(/*caret*/)); endmodule\n");
        assert_eq!(c.site, CompletionSite::NamedPortConnExpr);
    }

    #[test]
    fn detects_named_port_conn_expr_after_name_without_close_paren() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(.a(/*caret*/\nendmodule\n");
        assert_eq!(c.site, CompletionSite::NamedPortConnExpr);
    }

    #[test]
    fn detects_named_param_assign_expr_after_name() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(.W(/*caret*/)) u0(); endmodule\n",
        );
        assert_eq!(c.site, CompletionSite::NamedParamAssignExpr);
    }

    #[test]
    fn detects_hier_member_after_dot() {
        let c = ctx("module m; wire a; initial top.sub./*caret*/a; endmodule\n");
        assert_eq!(c.site, CompletionSite::MemberAccess);
    }

    #[test]
    fn detects_param_value_assignment_list() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(/*caret*/1) u0(); endmodule\n",
        );
        assert_eq!(c.site, CompletionSite::ParamValueAssignment);
    }

    #[test]
    fn detects_parameter_port_list() {
        let c = ctx("module m #(/*caret*/parameter W=1) (); endmodule\n");
        assert_eq!(c.site, CompletionSite::ParameterPortList);
    }

    #[test]
    fn detects_port_connection_list() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(/*caret*/); endmodule\n");
        assert_eq!(c.site, CompletionSite::PortConnections);
    }

    #[test]
    fn detects_port_connection_list_without_close_paren() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(/*caret*/\nendmodule\n");
        assert_eq!(c.site, CompletionSite::PortConnections);
    }

    #[test]
    fn detects_port_connection_list_trigger_open_paren_without_close_paren() {
        let c = ctx_with_trigger(
            "module m(input a); endmodule\nmodule top; m u0(/*caret*/\nendmodule\n",
            Some(TriggerChar::OpenParen),
        );
        assert_eq!(c.site, CompletionSite::PortConnections);
    }

    #[test]
    fn detects_argument_list() {
        let c = ctx("module m; initial f(/*caret*/); endmodule\n");
        assert_eq!(c.site, CompletionSite::Arguments);
    }

    #[test]
    fn detects_argument_list_trigger_open_paren_without_close_paren() {
        let c = ctx_with_trigger(
            "module m; initial f(/*caret*/\nendmodule\n",
            Some(TriggerChar::OpenParen),
        );
        assert_eq!(c.site, CompletionSite::Arguments);
    }

    #[test]
    fn detects_ansi_port_list() {
        let c = ctx("module m(input /*caret*/a); endmodule\n");
        assert_eq!(c.site, CompletionSite::Forbidden);
    }

    #[test]
    fn detects_ansi_port_list_trigger_comma() {
        let c = ctx_with_trigger(
            "module m(input a, /*caret*/b); endmodule\n",
            Some(TriggerChar::Comma),
        );
        assert_eq!(c.site, CompletionSite::Forbidden);
    }

    #[test]
    fn detects_non_ansi_port_list() {
        let c = ctx("module m(a, /*caret*/b); input a; output b; endmodule\n");
        assert_eq!(c.site, CompletionSite::NonAnsiPortList);
    }

    #[test]
    fn detects_after_at_trigger() {
        let c = ctx_with_trigger(
            "module m; always @/*caret*/(posedge clk) begin end endmodule\n",
            Some(TriggerChar::At),
        );
        assert_eq!(c.site, CompletionSite::AfterAtEventControl);
    }

    #[test]
    fn detects_after_backtick_trigger() {
        let c = ctx_with_trigger(
            "module m; initial `/*caret*/FOO; endmodule\n",
            Some(TriggerChar::Backtick),
        );
        assert_eq!(c.site, CompletionSite::PreprocDirective);
    }

    #[test]
    fn manual_and_triggered_backtick_use_same_site() {
        let text = "module m; initial `/*caret*/FOO; endmodule\n";
        let manual = ctx(text);
        let triggered = ctx_with_trigger(text, Some(TriggerChar::Backtick));
        assert_eq!(manual.site, triggered.site);
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
