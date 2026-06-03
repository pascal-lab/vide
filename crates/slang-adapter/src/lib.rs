use std::ops::Range;

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
    #[error("source range {start}..{end} is from buffer {buffer_id}, expected root buffer {root_buffer_id}")]
    NonRootSourceRange { buffer_id: u32, root_buffer_id: u32, start: usize, end: usize },
    #[error("source range {start}..{end} spans multiple buffers")]
    MultiBufferSourceRange { start: usize, end: usize },
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
        let range = self.node_range(node)?;
        if let Some(range) = &range {
            self.push_gap(range.start)?;
        }

        self.builder.start_node(map_node_kind(node.kind()).into());

        for child in node.children() {
            match child {
                slang::SyntaxElement::Node(child) => self.lower_node(child)?,
                slang::SyntaxElement::Token(token) => self.lower_token(token)?,
            }
        }

        if let Some(range) = &range {
            self.push_gap(range.end)?;
        }

        self.builder.finish_node();
        Ok(())
    }

    fn lower_token(&mut self, token: slang::SyntaxTokenWithParent<'_>) -> Result<(), RawSyntaxError> {
        let Some(range) = token.range() else {
            return Ok(());
        };
        let range = self.source_range(&range)?;
        self.push_gap(range.start)?;
        self.push_source(range, map_token_kind(token.kind()))?;
        Ok(())
    }

    fn node_range(
        &self,
        node: slang::SyntaxNode<'_>,
    ) -> Result<Option<Range<usize>>, RawSyntaxError> {
        node.range().map(|range| self.source_range(&range)).transpose()
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
        if self.cursor == end {
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
        self.source.get(range.clone()).ok_or(RawSyntaxError::InvalidSourceRange {
            start: range.start,
            end: range.end,
        })
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
    use syntax::raw::{AstNode, SourceFile};

    use super::*;

    #[test]
    fn converts_slang_parse_to_raw_tree() {
        let tree = parse_raw_syntax("module top; endmodule").unwrap();
        let file = SourceFile::cast(tree.root()).unwrap();
        let module = file.modules().next().unwrap();

        assert_eq!(tree.text(), "module top; endmodule");
        assert_eq!(module.name().unwrap().text(), "top");
    }
}
