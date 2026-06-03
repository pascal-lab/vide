use std::ops::Range;

use preproc::{CapabilityUnavailable, PreprocTrace, PreprocTraceResult};
use rowan::GreenNodeBuilder;
use syntax::raw::{RawSyntaxTree, SyntaxKind as RawSyntaxKind};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RawSyntaxError {
    #[error("slang parse did not produce a root syntax node")]
    MissingRoot,
    #[error("source range {start}..{end} is not valid UTF-8 boundary-aligned source text")]
    InvalidSourceRange { start: usize, end: usize },
    #[error("source range {start}..{end} appears before already emitted offset {cursor}")]
    OverlappingRange { start: usize, end: usize, cursor: usize },
    #[error(
        "source range {start}..{end} is from buffer {buffer_id}, expected root buffer {root_buffer_id}"
    )]
    NonRootSourceRange { buffer_id: u32, root_buffer_id: u32, start: usize, end: usize },
    #[error("source range {start}..{end} spans multiple buffers")]
    MultiBufferSourceRange { start: usize, end: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PreprocTraceInput {
    pub text: String,
    pub name: String,
    pub path: String,
    pub predefines: Vec<String>,
    pub include_paths: Vec<String>,
    pub include_buffers: Vec<PreprocTraceBuffer>,
    pub expand_includes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocTraceBuffer {
    pub path: String,
    pub text: String,
}

pub fn extract_preproc_trace(_input: &PreprocTraceInput) -> PreprocTraceResult<PreprocTrace> {
    PreprocTraceResult::CapabilityUnavailable(CapabilityUnavailable::binding_unavailable(
        "slang binding does not expose expansion trace",
    ))
}

pub fn parse_raw_syntax(text: &str) -> Result<RawSyntaxTree, RawSyntaxError> {
    let options = slang::SyntaxTreeOptions::without_include_expansion();
    let tree = slang::SyntaxTree::from_text_with_options(text, "source", "", &options);
    from_slang_tree(text, &tree)
}

pub fn from_slang_tree(
    source: &str,
    tree: &slang::SyntaxTree,
) -> Result<RawSyntaxTree, RawSyntaxError> {
    let root = tree.root().ok_or(RawSyntaxError::MissingRoot)?;
    let mut ctx = LowerCtx {
        source,
        root_buffer_id: tree.buffer_id(),
        cursor: 0,
        builder: GreenNodeBuilder::new(),
    };

    ctx.lower_node(root)?;
    ctx.push_gap(source.len())?;

    Ok(RawSyntaxTree::from_green(ctx.builder.finish()))
}

struct LowerCtx<'a> {
    source: &'a str,
    root_buffer_id: u32,
    cursor: usize,
    builder: GreenNodeBuilder<'static>,
}

impl LowerCtx<'_> {
    fn lower_node(&mut self, node: slang::SyntaxNode<'_>) -> Result<(), RawSyntaxError> {
        self.builder.start_node(map_node_kind(node.kind()).into());

        for child in node.children() {
            match child {
                slang::SyntaxElement::Node(child) => self.lower_node(child)?,
                slang::SyntaxElement::Token(token) => self.lower_token(token)?,
            }
        }

        self.builder.finish_node();
        Ok(())
    }

    fn lower_token(
        &mut self,
        token: slang::SyntaxTokenWithParent<'_>,
    ) -> Result<(), RawSyntaxError> {
        let Some(range) = token.range() else {
            return Ok(());
        };
        let range = self.source_range(&range)?;
        self.push_gap(range.start)?;
        self.push_source(range, map_token_kind(token.kind()))?;
        Ok(())
    }

    fn source_range(&self, range: &slang::SourceRange) -> Result<Range<usize>, RawSyntaxError> {
        if !range.is_single_buffer() {
            return Err(RawSyntaxError::MultiBufferSourceRange {
                start: range.start(),
                end: range.end(),
            });
        }

        let buffer_id = range.start_buffer_id();
        if buffer_id != self.root_buffer_id {
            return Err(RawSyntaxError::NonRootSourceRange {
                buffer_id,
                root_buffer_id: self.root_buffer_id,
                start: range.start(),
                end: range.end(),
            });
        }

        Ok(range.start()..range.end())
    }

    fn push_gap(&mut self, end: usize) -> Result<(), RawSyntaxError> {
        if self.cursor >= end {
            return Ok(());
        }

        self.push_source(self.cursor..end, gap_kind(self.source_slice(self.cursor..end)?))
    }

    fn push_source(
        &mut self,
        range: Range<usize>,
        kind: RawSyntaxKind,
    ) -> Result<(), RawSyntaxError> {
        if range.start < self.cursor {
            return Err(RawSyntaxError::OverlappingRange {
                start: range.start,
                end: range.end,
                cursor: self.cursor,
            });
        }

        if range.start > self.cursor {
            let gap = self.cursor..range.start;
            let text = self.source_slice(gap.clone())?.to_owned();
            let kind = gap_kind(&text);
            self.builder.token(kind.into(), &text);
            self.cursor = gap.end;
        }

        let text = self.source_slice(range.clone())?.to_owned();
        if !text.is_empty() {
            self.builder.token(kind.into(), &text);
        }
        self.cursor = range.end;
        Ok(())
    }

    fn source_slice(&self, range: Range<usize>) -> Result<&str, RawSyntaxError> {
        self.source
            .get(range.clone())
            .ok_or(RawSyntaxError::InvalidSourceRange { start: range.start, end: range.end })
    }
}

fn gap_kind(text: &str) -> RawSyntaxKind {
    let trimmed = text.trim_start();
    if text.chars().all(char::is_whitespace) {
        if text.contains('\n') || text.contains('\r') {
            RawSyntaxKind::EndOfLine
        } else {
            RawSyntaxKind::Whitespace
        }
    } else if trimmed.starts_with("//") || trimmed.starts_with("/*") {
        RawSyntaxKind::Comment
    } else {
        RawSyntaxKind::UnknownTrivia
    }
}

fn map_node_kind(kind: slang::SyntaxKind) -> RawSyntaxKind {
    match kind {
        slang::SyntaxKind::COMPILATION_UNIT => RawSyntaxKind::CompilationUnit,
        slang::SyntaxKind::SYNTAX_LIST => RawSyntaxKind::SyntaxList,
        slang::SyntaxKind::SEPARATED_LIST => RawSyntaxKind::SeparatedList,
        slang::SyntaxKind::MODULE_DECLARATION => RawSyntaxKind::ModuleDeclaration,
        slang::SyntaxKind::MODULE_HEADER => RawSyntaxKind::ModuleHeader,
        slang::SyntaxKind::ANSI_PORT_LIST => RawSyntaxKind::AnsiPortList,
        slang::SyntaxKind::IMPLICIT_ANSI_PORT => RawSyntaxKind::ImplicitAnsiPort,
        slang::SyntaxKind::VARIABLE_PORT_HEADER => RawSyntaxKind::VariablePortHeader,
        slang::SyntaxKind::IMPLICIT_TYPE => RawSyntaxKind::ImplicitType,
        slang::SyntaxKind::DATA_DECLARATION => RawSyntaxKind::DataDeclaration,
        slang::SyntaxKind::NET_TYPE_DECLARATION => RawSyntaxKind::NetType,
        slang::SyntaxKind::DECLARATOR => RawSyntaxKind::Declarator,
        slang::SyntaxKind::CONTINUOUS_ASSIGN => RawSyntaxKind::ContinuousAssign,
        slang::SyntaxKind::ASSIGNMENT_EXPRESSION => RawSyntaxKind::AssignmentExpression,
        slang::SyntaxKind::IDENTIFIER_NAME => RawSyntaxKind::NamedValueExpression,
        _ => RawSyntaxKind::UnknownNode,
    }
}

fn map_token_kind(kind: slang::TokenKind) -> RawSyntaxKind {
    match kind {
        slang::TokenKind::IDENTIFIER => RawSyntaxKind::Identifier,
        slang::TokenKind::SYSTEM_IDENTIFIER => RawSyntaxKind::SystemIdentifier,
        slang::TokenKind::STRING_LITERAL => RawSyntaxKind::StringLiteral,
        slang::TokenKind::INTEGER_LITERAL => RawSyntaxKind::IntegerLiteral,
        slang::TokenKind::REAL_LITERAL => RawSyntaxKind::RealLiteral,
        slang::TokenKind::TIME_LITERAL => RawSyntaxKind::TimeLiteral,
        slang::TokenKind::PLACEHOLDER => RawSyntaxKind::Placeholder,
        slang::TokenKind::MODULE_KEYWORD => RawSyntaxKind::ModuleKeyword,
        slang::TokenKind::END_MODULE_KEYWORD => RawSyntaxKind::EndModuleKeyword,
        slang::TokenKind::INPUT_KEYWORD => RawSyntaxKind::InputKeyword,
        slang::TokenKind::OUTPUT_KEYWORD => RawSyntaxKind::OutputKeyword,
        slang::TokenKind::IN_OUT_KEYWORD => RawSyntaxKind::InOutKeyword,
        slang::TokenKind::WIRE_KEYWORD => RawSyntaxKind::WireKeyword,
        slang::TokenKind::LOGIC_KEYWORD => RawSyntaxKind::LogicKeyword,
        slang::TokenKind::NET_TYPE_KEYWORD => RawSyntaxKind::NetType,
        slang::TokenKind::ASSIGN_KEYWORD => RawSyntaxKind::AssignKeyword,
        slang::TokenKind::SEMICOLON => RawSyntaxKind::Semicolon,
        slang::TokenKind::COLON => RawSyntaxKind::Colon,
        slang::TokenKind::COMMA => RawSyntaxKind::Comma,
        slang::TokenKind::DOT => RawSyntaxKind::Dot,
        slang::TokenKind::HASH => RawSyntaxKind::Hash,
        slang::TokenKind::EQUALS => RawSyntaxKind::Equals,
        slang::TokenKind::MINUS => RawSyntaxKind::Minus,
        slang::TokenKind::OPEN_PARENTHESIS => RawSyntaxKind::OpenParenthesis,
        slang::TokenKind::CLOSE_PARENTHESIS => RawSyntaxKind::CloseParenthesis,
        slang::TokenKind::OPEN_BRACKET => RawSyntaxKind::OpenBracket,
        slang::TokenKind::CLOSE_BRACKET => RawSyntaxKind::CloseBracket,
        _ => RawSyntaxKind::UnknownToken,
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use preproc::{PREPROC_TRACE_CAPABILITY, TraceUnavailableReason};
    use syntax::raw::{AstNode, SourceFile, SyntaxElement, SyntaxNode};

    use super::*;

    const SAMPLE: &str = "module top #(parameter WIDTH = 4) (input logic clk, output wire [WIDTH-1:0] data);\n  assign data = 4'd0;\nendmodule\n";

    #[test]
    fn converts_slang_parse_to_raw_tree() {
        let tree = parse_raw_syntax("module top; endmodule").unwrap();
        let file = SourceFile::cast(tree.root()).unwrap();
        let module = file.modules().next().unwrap();

        assert_eq!(tree.text(), "module top; endmodule");
        assert_eq!(module.name().unwrap().text(), "top");
    }

    #[test]
    fn raw_tree_dump_matches_snapshot() {
        let tree = parse_raw_syntax(SAMPLE).unwrap();

        expect![[r##"
            CompilationUnit 0..115
              SyntaxList 0..114
                ModuleDeclaration 0..114
                  SyntaxList 0..0
                  ModuleHeader 0..82
                    ModuleKeyword 0..6 "module"
                    Whitespace 6..7 " "
                    Identifier 7..10 "top"
                    SyntaxList 10..10
                    UnknownNode 10..33
                      Whitespace 10..11 " "
                      Hash 11..12 "#"
                      OpenParenthesis 12..13 "("
                      SeparatedList 13..32
                        UnknownNode 13..32
                          UnknownToken 13..22 "parameter"
                          ImplicitType 22..23
                            SyntaxList 22..22
                            Whitespace 22..23 " "
                          SeparatedList 23..32
                            Declarator 23..32
                              Identifier 23..28 "WIDTH"
                              SyntaxList 28..28
                              UnknownNode 28..32
                                Whitespace 28..29 " "
                                Equals 29..30 "="
                                UnknownNode 30..32
                                  Whitespace 30..31 " "
                                  IntegerLiteral 31..32 "4"
                      CloseParenthesis 32..33 ")"
                    AnsiPortList 33..81
                      Whitespace 33..34 " "
                      OpenParenthesis 34..35 "("
                      SeparatedList 35..80
                        ImplicitAnsiPort 35..50
                          SyntaxList 35..35
                          VariablePortHeader 35..46
                            InputKeyword 35..40 "input"
                            UnknownNode 40..46
                              Whitespace 40..41 " "
                              LogicKeyword 41..46 "logic"
                              SyntaxList 46..46
                          Declarator 46..50
                            Whitespace 46..47 " "
                            Identifier 47..50 "clk"
                            SyntaxList 50..50
                        Comma 50..51 ","
                        ImplicitAnsiPort 51..80
                          SyntaxList 51..51
                          UnknownNode 51..76
                            Whitespace 51..52 " "
                            OutputKeyword 52..58 "output"
                            Whitespace 58..59 " "
                            WireKeyword 59..63 "wire"
                            ImplicitType 63..76
                              SyntaxList 63..75
                                UnknownNode 63..75
                                  Whitespace 63..64 " "
                                  OpenBracket 64..65 "["
                                  UnknownNode 65..74
                                    UnknownNode 65..74
                                      UnknownNode 65..72
                                        NamedValueExpression 65..70
                                          Identifier 65..70 "WIDTH"
                                        Minus 70..71 "-"
                                        SyntaxList 71..71
                                        UnknownNode 71..72
                                          IntegerLiteral 71..72 "1"
                                      Colon 72..73 ":"
                                      UnknownNode 73..74
                                        IntegerLiteral 73..74 "0"
                                  CloseBracket 74..75 "]"
                              Whitespace 75..76 " "
                          Declarator 76..80
                            Identifier 76..80 "data"
                            SyntaxList 80..80
                      CloseParenthesis 80..81 ")"
                    Semicolon 81..82 ";"
                  SyntaxList 82..104
                    ContinuousAssign 82..104
                      SyntaxList 82..82
                      EndOfLine 82..85 "\n  "
                      AssignKeyword 85..91 "assign"
                      SeparatedList 91..103
                        AssignmentExpression 91..103
                          NamedValueExpression 91..96
                            Whitespace 91..92 " "
                            Identifier 92..96 "data"
                          Whitespace 96..97 " "
                          Equals 97..98 "="
                          SyntaxList 98..98
                          UnknownNode 98..103
                            Whitespace 98..99 " "
                            IntegerLiteral 99..100 "4"
                            UnknownToken 100..102 "'d"
                            IntegerLiteral 102..103 "0"
                      Semicolon 103..104 ";"
                  EndOfLine 104..105 "\n"
                  EndModuleKeyword 105..114 "endmodule"
              EndOfLine 114..115 "\n"
        "##]]
        .assert_eq(&tree.debug_dump());
    }

    #[test]
    fn token_texts_are_lossless_to_source() {
        let tree = parse_raw_syntax(SAMPLE).unwrap();
        let token_text = token_text(&tree.root());

        assert_eq!(token_text, SAMPLE);
    }

    #[test]
    fn node_and_token_ranges_match_source_text() {
        let tree = parse_raw_syntax(SAMPLE).unwrap();

        assert_ranges_match_source(&tree.root(), SAMPLE);
    }

    #[test]
    fn preproc_trace_api_reports_binding_unavailable() {
        let input = PreprocTraceInput {
            text: "`define WIDTH 8\n`WIDTH\n".to_owned(),
            name: "source".to_owned(),
            path: String::new(),
            predefines: Vec::new(),
            include_paths: Vec::new(),
            include_buffers: Vec::new(),
            expand_includes: true,
        };

        assert_eq!(
            extract_preproc_trace(&input),
            PreprocTraceResult::CapabilityUnavailable(CapabilityUnavailable {
                capability: PREPROC_TRACE_CAPABILITY.into(),
                reason: TraceUnavailableReason::BindingUnavailable {
                    reason: "slang binding does not expose expansion trace".into(),
                },
            })
        );
    }

    fn token_text(node: &SyntaxNode) -> String {
        let mut text = String::new();
        collect_token_text(node, &mut text);
        text
    }

    fn collect_token_text(node: &SyntaxNode, out: &mut String) {
        for child in node.children_with_tokens() {
            match child {
                rowan::NodeOrToken::Node(node) => collect_token_text(&node, out),
                rowan::NodeOrToken::Token(token) => out.push_str(token.text()),
            }
        }
    }

    fn assert_ranges_match_source(node: &SyntaxNode, source: &str) {
        let range = node.text_range();
        assert_eq!(
            &source[usize::from(range.start())..usize::from(range.end())],
            node.text().to_string()
        );

        for child in node.children_with_tokens() {
            match child {
                SyntaxElement::Node(node) => assert_ranges_match_source(&node, source),
                SyntaxElement::Token(token) => {
                    let range = token.text_range();
                    assert_eq!(
                        &source[usize::from(range.start())..usize::from(range.end())],
                        token.text()
                    );
                }
            }
        }
    }
}
