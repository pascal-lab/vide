//! Completion context detection.

mod caret;
mod decl_name;
mod expected;
mod lex;
mod parser;
mod resolve;
mod util;

use base_db::source_db::{SourceDb, SourceRootDb};
use hir::semantics::Semantics;
use ide_db::root_db::RootDb;
use smallvec::{SmallVec, smallvec};
use span::FilePosition;
use syntax::{ParserExpectedSyntax, SyntaxKeywordContext, SyntaxNode};
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
pub enum TriggerChar {
    Dot,
    OpenParen,
    Comma,
    At,
    Hash,
    Backtick,
    Newline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectedSyntax {
    DirectiveName,
    Keyword(SyntaxKeywordContext),
    Expression,
    PortConnectionName,
    ParameterAssignmentName,
    MemberName,
    PortConnectionExpr,
    ParameterAssignmentExpr,
    ElseClause,
    AfterParamValueAssignmentHash,
    AfterParameterPortListHash,
    ParamValueAssignment,
    ParameterPortListItem,
    PortConnection,
    ArgumentExpr,
    AnsiPortItem,
    FunctionPortItem,
    NonAnsiPortName,
    EventControl { wrap_in_parens: bool },
    DeclName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectationSource {
    DirectiveWord,
    Parser,
    DeclarationName,
    Ast(syntax::SyntaxKind),
    Token(syntax::TokenKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionExpectation {
    pub syntax: ExpectedSyntax,
    pub source: ExpectationSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionContext {
    pub replacement: TextRange,
    pub prefix: String,
    pub trigger: Option<TriggerChar>,
    pub lex: LexContext,
    pub expectations: SmallVec<[CompletionExpectation; 4]>,
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
    let Some(root) = sema.parse_root(file_id) else {
        return CompletionContext {
            replacement: TextRange::empty(offset),
            prefix: String::new(),
            trigger,
            lex: LexContext::Code,
            expectations: SmallVec::new(),
            in_decl_name: false,
        };
    };
    let text = db.file_text(file_id);
    let parser_expected_syntax = db.parser_expected_syntax(file_id, offset);
    detect_completion_context_impl(
        root,
        offset,
        trigger,
        Some(&text),
        Some(&parser_expected_syntax),
    )
}

pub fn detect_completion_context(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    detect_completion_context_impl(root, offset, trigger, None, None)
}

pub fn detect_completion_context_with_source_text(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    source_text: &str,
) -> CompletionContext {
    let parser_expected_syntax = parser_expected_syntax_for_text(root, source_text, offset);
    detect_completion_context_impl(
        root,
        offset,
        trigger,
        Some(source_text),
        Some(&parser_expected_syntax),
    )
}

fn detect_completion_context_impl(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    source_text: Option<&str>,
    parser_expected_syntax: Option<&[ParserExpectedSyntax]>,
) -> CompletionContext {
    let caret = CaretSnapshot::new(root, offset);
    let (mut replacement, mut prefix) = caret.replacement_and_prefix();

    let lex = lex::detect_lex_context(&caret);
    if lex != LexContext::Code {
        let expectation = if lex == LexContext::PreprocDirective
            && let Some(word) = directive_word_at_offset(source_text, offset)
        {
            replacement = word.replacement;
            prefix = word.prefix;
            Some(CompletionExpectation {
                syntax: ExpectedSyntax::DirectiveName,
                source: ExpectationSource::DirectiveWord,
            })
        } else {
            None
        };
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            expectations: expectation.into_iter().collect(),
            in_decl_name: false,
        };
    }

    if let Some(word) = directive_word_at_offset(source_text, offset) {
        replacement = word.replacement;
        prefix = word.prefix;
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            expectations: smallvec![CompletionExpectation {
                syntax: ExpectedSyntax::DirectiveName,
                source: ExpectationSource::DirectiveWord,
            }],
            in_decl_name: false,
        };
    }

    if prefix.is_empty()
        && let Some(word) = identifier_word_at_offset(source_text, offset)
    {
        replacement = word.replacement;
        prefix = word.prefix;
    }

    let parser = parser::expectations(parser_expected_syntax);
    let in_decl_name = decl_name::is_in_decl_name(&caret, parser.has_decl_name());
    let local = expected::detect_local(&caret);
    let expectations = resolve::expectations(parser, local, in_decl_name, &prefix, trigger);
    CompletionContext { replacement, prefix, trigger, lex, expectations, in_decl_name }
}

fn parser_expected_syntax_for_text(
    root: SyntaxNode<'_>,
    source_text: &str,
    offset: TextSize,
) -> Vec<ParserExpectedSyntax> {
    parser::parser_expected_syntax_for_text(root, source_text, offset)
}

fn directive_word_at_offset(source_text: Option<&str>, offset: TextSize) -> Option<DirectiveWord> {
    let source_text = source_text?;
    let offset = usize::from(offset);
    if offset == 0 || offset > source_text.len() || !source_text.is_char_boundary(offset) {
        return None;
    }

    let bytes = source_text.as_bytes();
    let mut start = offset;
    while start > 0 && bytes.get(start - 1).is_some_and(|byte| is_identifier_name_byte(*byte)) {
        start -= 1;
    }

    if start == 0 || bytes.get(start - 1) != Some(&b'`') {
        return None;
    }

    let mut end = offset;
    while bytes.get(end).is_some_and(|byte| is_identifier_name_byte(*byte)) {
        end += 1;
    }

    let prefix = source_text[start..offset].to_string();
    let replacement = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    Some(DirectiveWord { replacement, prefix })
}

fn identifier_word_at_offset(source_text: Option<&str>, offset: TextSize) -> Option<DirectiveWord> {
    let source_text = source_text?;
    let offset = usize::from(offset);
    if offset == 0 || offset > source_text.len() || !source_text.is_char_boundary(offset) {
        return None;
    }

    let bytes = source_text.as_bytes();
    let mut start = offset;
    while start > 0 && bytes.get(start - 1).is_some_and(|byte| is_identifier_name_byte(*byte)) {
        start -= 1;
    }

    if start == offset {
        return None;
    }

    let mut end = offset;
    while bytes.get(end).is_some_and(|byte| is_identifier_name_byte(*byte)) {
        end += 1;
    }

    let prefix = source_text[start..offset].to_string();
    let replacement = TextRange::new(TextSize::from(start as u32), TextSize::from(end as u32));
    Some(DirectiveWord { replacement, prefix })
}

fn is_identifier_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
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
        ctx_with_trigger(text, None)
    }

    fn library_map_ctx(text: &str) -> CompletionContext {
        let marker = "/*caret*/";
        let off = text.find(marker).expect("missing /*caret*/");
        let owned = text.replace(marker, "");
        let tree = SyntaxTree::from_library_map_text(&owned, "test", "test.map");
        let root = tree.root().unwrap();
        detect_completion_context_with_source_text(root, TextSize::from(off as u32), None, &owned)
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

        let root = tree.root().unwrap();
        detect_completion_context_with_source_text(
            root,
            TextSize::from(off as u32),
            trigger,
            &owned,
        )
    }

    fn expected(c: &CompletionContext) -> Option<ExpectedSyntax> {
        c.expectations.first().map(|expectation| expectation.syntax)
    }

    fn source(c: &CompletionContext, syntax: ExpectedSyntax) -> Option<ExpectationSource> {
        c.expectations
            .iter()
            .find(|expectation| expectation.syntax == syntax)
            .map(|expectation| expectation.source)
    }

    fn keyword(context: SyntaxKeywordContext) -> ExpectedSyntax {
        ExpectedSyntax::Keyword(context)
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
        assert_eq!(expected(&c), None);
    }

    #[test]
    fn detects_preproc_directive_at_boundary() {
        let c = ctx("`define FOO/*caret*/\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::PreprocDirective);
        assert_eq!(expected(&c), None);
    }

    #[test]
    fn detects_preproc_directive_keyword() {
        let c = ctx("`de/*caret*/fine FOO 1\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::Code);
        assert_eq!(expected(&c), Some(ExpectedSyntax::DirectiveName));
        assert_eq!(c.prefix, "de");
    }

    #[test]
    fn normalizes_preproc_directive_word_replacement() {
        let c = ctx("`de/*caret*/fine FOO 1\nmodule m; endmodule\n");
        assert_eq!(c.lex, LexContext::Code);
        assert_eq!(expected(&c), Some(ExpectedSyntax::DirectiveName));
        assert_eq!(c.prefix, "de");
        assert_eq!(c.replacement, TextRange::new(TextSize::from(1), TextSize::from(7)));
    }

    #[test]
    fn detects_inline_preproc_directive_word() {
        let c = ctx("module m; initial `de/*caret*/; endmodule\n");
        assert_eq!(c.lex, LexContext::Code);
        assert_eq!(expected(&c), Some(ExpectedSyntax::DirectiveName));
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
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnectionName));
    }

    #[test]
    fn detects_named_port_after_dot_without_name() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(./*caret*/); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnectionName));
    }

    #[test]
    fn detects_named_param_after_dot() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(./*caret*/W(1)) u0(); endmodule\n",
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::ParameterAssignmentName));
    }

    #[test]
    fn detects_named_param_after_dot_without_name() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(./*caret*/) u0(); endmodule\n",
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::ParameterAssignmentName));
    }

    #[test]
    fn detects_named_port_conn_expr_after_name() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(.a(/*caret*/)); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnectionExpr));
    }

    #[test]
    fn detects_named_port_conn_expr_after_name_without_close_paren() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(.a(/*caret*/\nendmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnectionExpr));
    }

    #[test]
    fn detects_named_param_assign_expr_after_name() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(.W(/*caret*/)) u0(); endmodule\n",
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::ParameterAssignmentExpr));
    }

    #[test]
    fn detects_hier_member_after_dot() {
        let c = ctx("module m; wire a; initial top.sub./*caret*/a; endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::MemberName));
    }

    #[test]
    fn detects_param_value_assignment_list() {
        let c = ctx(
            "module m #(parameter W=1) (); endmodule\nmodule top; m #(/*caret*/1) u0(); endmodule\n",
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::ParamValueAssignment));
    }

    #[test]
    fn detects_parameter_port_list() {
        let c = ctx("module m #(/*caret*/parameter W=1) (); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::ParameterPortListItem));
    }

    #[test]
    fn detects_parameter_port_keyword_prefix_over_decl_name() {
        let c = ctx("module m #(para/*caret*/) (); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::ParameterPortListItem));
        assert_eq!(c.prefix, "para");
    }

    #[test]
    fn detects_port_connection_list() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(/*caret*/); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnection));
    }

    #[test]
    fn detects_port_connection_list_without_close_paren() {
        let c = ctx("module m(input a); endmodule\nmodule top; m u0(/*caret*/\nendmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnection));
    }

    #[test]
    fn detects_port_connection_list_trigger_open_paren_without_close_paren() {
        let c = ctx_with_trigger(
            "module m(input a); endmodule\nmodule top; m u0(/*caret*/\nendmodule\n",
            Some(TriggerChar::OpenParen),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::PortConnection));
    }

    #[test]
    fn detects_argument_list() {
        let c = ctx("module m; initial f(/*caret*/); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::ArgumentExpr));
    }

    #[test]
    fn detects_argument_list_trigger_open_paren_without_close_paren() {
        let c = ctx_with_trigger(
            "module m; initial f(/*caret*/\nendmodule\n",
            Some(TriggerChar::OpenParen),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::ArgumentExpr));
    }

    #[test]
    fn detects_ansi_port_list() {
        let c = ctx("module m(input /*caret*/a); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::DeclName));
    }

    #[test]
    fn detects_ansi_port_list_trigger_comma() {
        let c = ctx_with_trigger(
            "module m(input a, /*caret*/b); endmodule\n",
            Some(TriggerChar::Comma),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::DeclName));
    }

    #[test]
    fn detects_ansi_port_keyword_prefix_at_first_port() {
        let c = ctx("module top\n(\n  in/*caret*/\n);\nendmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::AnsiPortItem));
        assert_eq!(c.prefix, "in");
    }

    #[test]
    fn detects_ansi_port_keyword_prefix_after_comma() {
        let c = ctx("module m(input a, o/*caret*/); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::AnsiPortItem));
        assert_eq!(c.prefix, "o");
    }

    #[test]
    fn detects_ansi_port_keyword_prefix_over_decl_name() {
        let c = ctx("module m(input wir/*caret*/); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::AnsiPortItem));
        assert_eq!(c.prefix, "wir");
    }

    #[test]
    fn detects_ansi_port_list_after_newline_trigger() {
        let c = ctx_with_trigger(
            "module top\n(\n  /*caret*/\n);\nendmodule\n",
            Some(TriggerChar::Newline),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::AnsiPortItem));
    }

    #[test]
    fn detects_function_port_keyword_prefix() {
        let c = ctx("module m; task t(in/*caret*/); endtask endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::FunctionPortItem));
        assert_eq!(c.prefix, "in");
    }

    #[test]
    fn detects_function_port_list_after_newline_trigger() {
        let c = ctx_with_trigger(
            "module m;\ntask t(\n  /*caret*/\n);\nendtask\nendmodule\n",
            Some(TriggerChar::Newline),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::FunctionPortItem));
    }

    #[test]
    fn keeps_non_keyword_ansi_port_decl_name_forbidden() {
        let c = ctx("module m(input a, b/*caret*/); endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::DeclName));
    }

    #[test]
    fn detects_non_ansi_port_list() {
        let c = ctx("module m(a, /*caret*/b); input a; output b; endmodule\n");
        assert_eq!(expected(&c), Some(ExpectedSyntax::NonAnsiPortName));
    }

    #[test]
    fn detects_top_level_item_start() {
        let c = ctx("module m; endmodule\n/*caret*/\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::CompilationUnitMember)));
    }

    #[test]
    fn detects_top_level_item_keyword_prefix() {
        for text in ["con/*caret*/\n", "pri/*caret*/\n"] {
            let c = ctx(text);
            assert_eq!(
                expected(&c),
                Some(keyword(SyntaxKeywordContext::CompilationUnitMember)),
                "{text}"
            );
            assert!(!c.in_decl_name, "{text}");
        }
    }

    #[test]
    fn detects_library_map_item_keyword_prefix() {
        let c = library_map_ctx("lib/*caret*/\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::LibraryMapMember)));
        assert_eq!(c.prefix, "lib");
    }

    #[test]
    fn detects_module_member_start() {
        let c = ctx("module m;\n  /*caret*/\nendmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::ModuleMember)));
    }

    #[test]
    fn detects_generate_region_item_start() {
        let c = ctx("module m; generate\n  /*caret*/\nendgenerate endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::GenerateMember)));
    }

    #[test]
    fn detects_generate_block_item_start() {
        let c = ctx("module m; begin : g\n  /*caret*/\nend endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::GenerateMember)));
    }

    #[test]
    fn detects_specify_item_start() {
        let c = ctx("module m; specify\n  /*caret*/\nendspecify endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::SpecifyItem)));
    }

    #[test]
    fn detects_specify_item_keyword_prefix() {
        let c = ctx("module m; specify\n  sp/*caret*/\nendspecify endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::SpecifyItem)));
    }

    #[test]
    fn detects_config_header_item_start() {
        let c = ctx("config cfg;\n  de/*caret*/\n  design work.top;\nendconfig\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::ConfigHeaderItem)));
    }

    #[test]
    fn detects_config_rule_item_start() {
        let c = ctx("config cfg;\n  design work.top;\n  de/*caret*/\nendconfig\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::ConfigRule)));
    }

    #[test]
    fn item_context_recovery_uses_identifier_replacement_start() {
        let c = ctx("module m; specify\n  sp/*caret*/\nendspecify endmodule\n");

        assert_eq!(c.replacement, TextRange::new(TextSize::from(20), TextSize::from(22)));
        assert_eq!(c.prefix, "sp");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::SpecifyItem)));
        assert_eq!(
            c.expectations.first().map(|expectation| expectation.source),
            Some(ExpectationSource::Parser)
        );
    }

    #[test]
    fn broad_completion_contexts_come_from_parser_expected_syntax() {
        let cases = [
            ("module m;\n  /*caret*/\nendmodule\n", keyword(SyntaxKeywordContext::ModuleMember)),
            (
                "module m; initial begin\n  /*caret*/\nend endmodule\n",
                keyword(SyntaxKeywordContext::BlockItem),
            ),
            (
                "module m; initial begin\n  x = 1;\n  /*caret*/\nend endmodule\n",
                keyword(SyntaxKeywordContext::Statement),
            ),
            ("module m; logic [7:0] lhs = /*caret*/; endmodule\n", ExpectedSyntax::Expression),
            ("module m #(\n  /*caret*/\n) (); endmodule\n", ExpectedSyntax::ParameterPortListItem),
            (
                "module m(input a); endmodule\nmodule top; m u0(/*caret*/); endmodule\n",
                ExpectedSyntax::PortConnection,
            ),
            ("module m; initial f(/*caret*/); endmodule\n", ExpectedSyntax::ArgumentExpr),
            ("module m(input a,\n  /*caret*/\n); endmodule\n", ExpectedSyntax::AnsiPortItem),
            (
                "module m; task t(input a,\n  /*caret*/\n); endtask endmodule\n",
                ExpectedSyntax::FunctionPortItem,
            ),
            (
                "module m(a, /*caret*/b); input a; output b; endmodule\n",
                ExpectedSyntax::NonAnsiPortName,
            ),
        ];

        for (text, syntax) in cases {
            let c = ctx(text);
            assert_eq!(source(&c, syntax), Some(ExpectationSource::Parser), "{text}");
        }
    }

    #[test]
    fn truncated_module_member_prefix_uses_parser_expected_syntax() {
        let c = ctx("module counter;\n  wi/*caret*/");

        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::ModuleMember)));
        assert_eq!(
            source(&c, keyword(SyntaxKeywordContext::ModuleMember)),
            Some(ExpectationSource::Parser)
        );
    }

    #[test]
    fn detects_block_decl_start_before_statement() {
        let c = ctx("module m; initial begin\n  /*caret*/\nend endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::BlockItem)));
    }

    #[test]
    fn detects_procedural_statement_start_after_statement() {
        let c = ctx("module m; initial begin\n  x = 1;\n  /*caret*/\nend endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::Statement)));
    }

    #[test]
    fn detects_block_decl_keyword_prefix_before_statement() {
        let c = ctx("module m; initial begin\n  re/*caret*/\nend endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::BlockItem)));
    }

    #[test]
    fn detects_procedural_statement_keyword_prefix_after_statement() {
        let c = ctx("module m; initial begin\n  x = 1;\n  re/*caret*/\nend endmodule\n");
        assert_eq!(expected(&c), Some(keyword(SyntaxKeywordContext::Statement)));
    }

    #[test]
    fn detects_else_clause_after_if_block() {
        let c = ctx(
            "module m; initial begin\n  if (cond) begin\n  end\n  el/*caret*/\nend endmodule\n",
        );
        assert!(
            c.expectations
                .iter()
                .any(|expectation| expectation.syntax == ExpectedSyntax::ElseClause)
        );
        assert_eq!(c.prefix, "el");
    }

    #[test]
    fn detects_after_at_trigger() {
        let c = ctx_with_trigger(
            "module m; always @/*caret*/(posedge clk) begin end endmodule\n",
            Some(TriggerChar::At),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::EventControl { wrap_in_parens: true }));
    }

    #[test]
    fn detects_after_backtick_trigger() {
        let c = ctx_with_trigger(
            "module m; initial `/*caret*/FOO; endmodule\n",
            Some(TriggerChar::Backtick),
        );
        assert_eq!(expected(&c), Some(ExpectedSyntax::DirectiveName));
    }

    #[test]
    fn manual_and_triggered_backtick_use_same_expectation() {
        let text = "module m; initial `/*caret*/FOO; endmodule\n";
        let manual = ctx(text);
        let triggered = ctx_with_trigger(text, Some(TriggerChar::Backtick));
        assert_eq!(expected(&manual), expected(&triggered));
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
