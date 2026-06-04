use std::{collections::VecDeque, ops::Range};

mod kind_map;

use preproc::{
    CapabilityUnavailable, PreprocTrace, PreprocTraceResult,
    directive_index::{MacroInclude, PreprocFileIndex},
};
use rowan::GreenNodeBuilder;
use syntax::{
    DiagnosticSeverity, LexedTokenAtOffset, OwnedDirectiveTrivia, OwnedTrivia,
    ParserExpectedSyntax, PreprocessorDirective, PreprocessorDirectiveToken,
    PreprocessorMacroParam, SourceBufferId, SyntaxDiagnostic, SyntaxKeywordContext, SyntaxTree,
    SyntaxTreeBufferIds, SyntaxTreeBuilder, SyntaxTreeOptions, TriviaKind,
    raw::{RawSyntaxTree, SyntaxKind as RawSyntaxKind},
};
use thiserror::Error;
use utils::line_index::{TextRange, TextSize};

use crate::kind_map::{owned_node_kind, owned_token_kind};

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

pub fn parse_syntax(text: &str, name: &str, path: &str) -> Result<SyntaxTree, RawSyntaxError> {
    parse_syntax_with_options(text, name, path, &SyntaxTreeOptions::default())
}

pub fn parse_syntax_with_options(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
) -> Result<SyntaxTree, RawSyntaxError> {
    let slang_options = to_slang_syntax_tree_options(options);
    let tree = slang::SyntaxTree::from_text_with_options(text, name, path, &slang_options);
    let directive_trivia =
        slang::SyntaxTree::preprocessor_directives(text, name, path, &slang_options)
            .into_iter()
            .filter_map(owned_directive_trivia_from_preprocessor)
            .collect();
    from_slang_syntax_tree_with_directives(text, &tree, directive_trivia)
}

pub struct ParsedSyntax {
    pub tree: SyntaxTree,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

pub fn parse_syntax_with_diagnostics(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
    warning_options: &[String],
) -> Result<ParsedSyntax, RawSyntaxError> {
    let slang_options = to_slang_syntax_tree_options(options);
    let tree = slang::SyntaxTree::from_text_with_options(text, name, path, &slang_options);
    let diagnostics =
        tree.diagnostics_with_options(warning_options).into_iter().map(owned_diagnostic).collect();
    let directive_trivia =
        slang::SyntaxTree::preprocessor_directives(text, name, path, &slang_options)
            .into_iter()
            .filter_map(owned_directive_trivia_from_preprocessor)
            .collect();
    let tree = from_slang_syntax_tree_with_directives(text, &tree, directive_trivia)?;
    Ok(ParsedSyntax { tree, diagnostics })
}

pub fn parse_library_map_syntax(
    text: &str,
    name: &str,
    path: &str,
) -> Result<SyntaxTree, RawSyntaxError> {
    let tree = slang::SyntaxTree::from_library_map_text(text, name, path);
    from_slang_syntax_tree(text, &tree)
}

pub fn parse_library_map_syntax_with_diagnostics(
    text: &str,
    name: &str,
    path: &str,
    warning_options: &[String],
) -> Result<ParsedSyntax, RawSyntaxError> {
    let tree = slang::SyntaxTree::from_library_map_text(text, name, path);
    let diagnostics =
        tree.diagnostics_with_options(warning_options).into_iter().map(owned_diagnostic).collect();
    let tree = from_slang_syntax_tree(text, &tree)?;
    Ok(ParsedSyntax { tree, diagnostics })
}

pub fn from_slang_syntax_tree(
    source: &str,
    tree: &slang::SyntaxTree,
) -> Result<SyntaxTree, RawSyntaxError> {
    from_slang_syntax_tree_with_directives(source, tree, VecDeque::new())
}

fn from_slang_syntax_tree_with_directives(
    source: &str,
    tree: &slang::SyntaxTree,
    directive_trivia: VecDeque<OwnedDirectiveTrivia>,
) -> Result<SyntaxTree, RawSyntaxError> {
    let root = tree.root().ok_or(RawSyntaxError::MissingRoot)?;
    let mut ctx = OwnedLowerCtx {
        root_buffer_id: tree.buffer_id(),
        builder: SyntaxTreeBuilder::new(source.to_owned(), tree.buffer_id()),
        directive_trivia,
    };
    ctx.lower_root(root);
    Ok(SyntaxTree::from_builder(ctx.builder))
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

struct OwnedLowerCtx {
    root_buffer_id: u32,
    builder: SyntaxTreeBuilder,
    directive_trivia: VecDeque<OwnedDirectiveTrivia>,
}

impl OwnedLowerCtx {
    fn lower_root(&mut self, node: slang::SyntaxNode<'_>) {
        self.builder.start_root(
            owned_node_kind(node.kind()),
            node.child_count(),
            self.node_range(node),
        );
        self.lower_children(node);
        self.builder.finish_node();
    }

    fn lower_child_node(&mut self, slot: usize, node: slang::SyntaxNode<'_>) {
        self.builder.start_child_node(
            slot,
            owned_node_kind(node.kind()),
            node.child_count(),
            self.node_range(node),
        );
        self.lower_children(node);
        self.builder.finish_node();
    }

    fn lower_children(&mut self, node: slang::SyntaxNode<'_>) {
        for slot in 0..node.child_count() {
            if let Some(child) = node.child_node(slot) {
                self.lower_child_node(slot, child);
            } else if let Some(token) = node.child_token(slot) {
                self.lower_token(slot, token);
            }
        }
    }

    fn lower_token(&mut self, slot: usize, token: slang::SyntaxToken<'_>) {
        let range = token.range().and_then(|range| self.text_range(&range));
        let trivia = token
            .trivias_with_loc()
            .filter_map(|(loc, trivia)| {
                let range = (loc.buffer_id == self.root_buffer_id)
                    .then(|| text_range(loc.start..loc.end))
                    .flatten();
                Some(OwnedTrivia {
                    kind: owned_trivia_kind(trivia.kind()),
                    raw_text: trivia.get_raw_text().to_string().into(),
                    range,
                    directive: self.lower_directive_trivia_payload(trivia),
                })
            })
            .collect();
        self.builder.token(
            slot,
            owned_token_kind(token.kind()),
            token.raw_text().to_string(),
            token.value_text().to_string(),
            range,
            trivia,
        );
    }

    fn node_range(&self, node: slang::SyntaxNode<'_>) -> Option<TextRange> {
        node.range().and_then(|range| self.text_range(&range))
    }

    fn text_range(&self, range: &slang::SourceRange) -> Option<TextRange> {
        if !range.is_single_buffer() || range.start_buffer_id() != self.root_buffer_id {
            return None;
        }
        text_range(range.start()..range.end())
    }

    fn lower_directive_trivia(
        &self,
        trivia: slang::SyntaxTrivia<'_>,
    ) -> Option<OwnedDirectiveTrivia> {
        if trivia.kind() != slang::TriviaKind::DIRECTIVE {
            return None;
        }

        let node = trivia.syntax()?;
        let first_token_trivia = node
            .first_token()
            .into_iter()
            .flat_map(|token| {
                token.tok.trivias_with_loc().filter_map(|(loc, trivia)| {
                    let range = (loc.buffer_id == self.root_buffer_id)
                        .then(|| text_range(loc.start..loc.end))
                        .flatten();
                    Some(OwnedTrivia {
                        kind: owned_trivia_kind(trivia.kind()),
                        raw_text: trivia.get_raw_text().to_string().into(),
                        range,
                        directive: None,
                    })
                })
            })
            .collect();

        Some(OwnedDirectiveTrivia {
            kind: owned_node_kind(node.kind()),
            range: self.node_range(node),
            first_token_trivia,
        })
    }

    fn lower_directive_trivia_payload(
        &mut self,
        trivia: slang::SyntaxTrivia<'_>,
    ) -> Option<OwnedDirectiveTrivia> {
        if trivia.kind() != slang::TriviaKind::DIRECTIVE {
            return None;
        }

        let local = self.lower_directive_trivia(trivia);
        let queued = self.directive_trivia.pop_front();
        match (local, queued) {
            (Some(mut local), Some(queued)) => {
                local.range = local.range.or(queued.range);
                if local.first_token_trivia.is_empty() {
                    local.first_token_trivia = queued.first_token_trivia;
                }
                Some(local)
            }
            (Some(local), None) => Some(local),
            (None, Some(queued)) => Some(queued),
            (None, None) => None,
        }
    }
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

pub fn parse_diagnostics_with_options(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
    warning_options: &[String],
) -> Vec<SyntaxDiagnostic> {
    let tree = slang::SyntaxTree::from_text_with_options(
        text,
        name,
        path,
        &to_slang_syntax_tree_options(options),
    );
    tree.diagnostics_with_options(warning_options).into_iter().map(owned_diagnostic).collect()
}

pub fn expected_syntax_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Vec<ParserExpectedSyntax> {
    expected_syntax_at_offset_with_options(text, name, path, offset, &SyntaxTreeOptions::default())
}

pub fn expected_syntax_at_offset_with_options(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
    options: &SyntaxTreeOptions,
) -> Vec<ParserExpectedSyntax> {
    slang::SyntaxTree::expected_syntax_at_offset_with_options(
        text,
        name,
        path,
        offset,
        &to_slang_syntax_tree_options(options),
    )
    .into_iter()
    .map(owned_expected_syntax)
    .collect()
}

pub fn library_map_expected_syntax_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Vec<ParserExpectedSyntax> {
    slang::SyntaxTree::library_map_expected_syntax_at_offset(text, name, path, offset)
        .into_iter()
        .map(owned_expected_syntax)
        .collect()
}

pub fn directive_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Option<LexedTokenAtOffset> {
    slang::SyntaxTree::directive_at_offset(text, name, path, offset).map(owned_lexed_token)
}

pub fn token_word_at_offset(
    text: &str,
    name: &str,
    path: &str,
    offset: usize,
) -> Option<LexedTokenAtOffset> {
    slang::SyntaxTree::token_word_at_offset(text, name, path, offset).map(owned_lexed_token)
}

pub fn preprocessor_directives(
    text: &str,
    name: &str,
    path: &str,
    options: &SyntaxTreeOptions,
) -> Vec<PreprocessorDirective> {
    slang::SyntaxTree::preprocessor_directives(
        text,
        name,
        path,
        &to_slang_syntax_tree_options(options),
    )
    .into_iter()
    .map(owned_preprocessor_directive)
    .collect()
}

pub fn preproc_file_index_from_text(text: &str, options: &SyntaxTreeOptions) -> PreprocFileIndex {
    let directives = preprocessor_directives(text, "source", "", options);
    preproc::directive_index::preproc_file_index_from_directives(directives, text)
}

pub fn literal_include_directives(text: &str) -> Vec<MacroInclude> {
    let index = preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion());
    preproc::directive_index::literal_include_directives_from_index(&index)
}

pub fn system_function_names() -> Vec<String> {
    slang::Compilation::system_function_names()
}

pub fn system_task_names() -> Vec<String> {
    slang::Compilation::system_task_names()
}

pub struct Compilation {
    inner: slang::Compilation,
}

impl Default for Compilation {
    fn default() -> Self {
        Self::new()
    }
}

impl Compilation {
    pub fn new() -> Self {
        Self { inner: slang::Compilation::new() }
    }

    pub fn new_with_top_modules(top_modules: &[String]) -> Self {
        Self { inner: slang::Compilation::new_with_top_modules(top_modules) }
    }

    pub fn add_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTreeBufferIds {
        owned_buffer_ids(self.inner.add_syntax_tree_from_text(
            text,
            name,
            path,
            &to_slang_syntax_tree_options(options),
        ))
    }

    pub fn add_library_map_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
    ) -> SyntaxTreeBufferIds {
        owned_buffer_ids(self.inner.add_library_map_syntax_tree_from_text(text, name, path))
    }

    pub fn parse_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        self.inner
            .parse_diagnostics_with_options(warning_options)
            .into_iter()
            .map(owned_diagnostic)
            .collect()
    }

    pub fn semantic_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        self.inner
            .semantic_diagnostics_with_options(warning_options)
            .into_iter()
            .map(owned_diagnostic)
            .collect()
    }
}

fn to_slang_syntax_tree_options(options: &SyntaxTreeOptions) -> slang::SyntaxTreeOptions {
    slang::SyntaxTreeOptions {
        predefines: options.predefines.clone(),
        include_paths: options.include_paths.clone(),
        include_buffers: options
            .include_buffers
            .iter()
            .map(|buffer| slang::SyntaxTreeBuffer {
                path: buffer.path.clone(),
                text: buffer.text.clone(),
            })
            .collect(),
        expand_includes: options.expand_includes,
    }
}

fn text_range(range: Range<usize>) -> Option<TextRange> {
    let start = u32::try_from(range.start).ok()?;
    let end = u32::try_from(range.end).ok()?;
    (start <= end).then(|| TextRange::new(TextSize::new(start), TextSize::new(end)))
}

fn owned_trivia_kind(kind: slang::TriviaKind) -> TriviaKind {
    match kind {
        slang::TriviaKind::UNKNOWN => TriviaKind::UNKNOWN,
        slang::TriviaKind::WHITESPACE => TriviaKind::WHITESPACE,
        slang::TriviaKind::END_OF_LINE => TriviaKind::END_OF_LINE,
        slang::TriviaKind::LINE_COMMENT => TriviaKind::LINE_COMMENT,
        slang::TriviaKind::BLOCK_COMMENT => TriviaKind::BLOCK_COMMENT,
        slang::TriviaKind::DISABLED_TEXT => TriviaKind::DISABLED_TEXT,
        slang::TriviaKind::SKIPPED_TOKENS => TriviaKind::SKIPPED_TOKENS,
        slang::TriviaKind::SKIPPED_SYNTAX => TriviaKind::SKIPPED_SYNTAX,
        slang::TriviaKind::DIRECTIVE => TriviaKind::DIRECTIVE,
        _ => TriviaKind::UNKNOWN,
    }
}

fn owned_keyword_context(context: slang::SyntaxKeywordContext) -> SyntaxKeywordContext {
    match context {
        slang::SyntaxKeywordContext::CompilationUnitMember => {
            SyntaxKeywordContext::CompilationUnitMember
        }
        slang::SyntaxKeywordContext::LibraryMapMember => SyntaxKeywordContext::LibraryMapMember,
        slang::SyntaxKeywordContext::ModuleHeaderItem => SyntaxKeywordContext::ModuleHeaderItem,
        slang::SyntaxKeywordContext::ModuleMember => SyntaxKeywordContext::ModuleMember,
        slang::SyntaxKeywordContext::GenerateMember => SyntaxKeywordContext::GenerateMember,
        slang::SyntaxKeywordContext::SpecifyItem => SyntaxKeywordContext::SpecifyItem,
        slang::SyntaxKeywordContext::ConfigHeaderItem => SyntaxKeywordContext::ConfigHeaderItem,
        slang::SyntaxKeywordContext::ConfigRule => SyntaxKeywordContext::ConfigRule,
        slang::SyntaxKeywordContext::BlockItem => SyntaxKeywordContext::BlockItem,
        slang::SyntaxKeywordContext::Statement => SyntaxKeywordContext::Statement,
        slang::SyntaxKeywordContext::ParameterPortListItem => {
            SyntaxKeywordContext::ParameterPortListItem
        }
        slang::SyntaxKeywordContext::AnsiPortItem => SyntaxKeywordContext::AnsiPortItem,
        slang::SyntaxKeywordContext::FunctionPortItem => SyntaxKeywordContext::FunctionPortItem,
        slang::SyntaxKeywordContext::GateType => SyntaxKeywordContext::GateType,
    }
}

fn owned_diagnostic(diag: slang::SyntaxDiagnostic) -> SyntaxDiagnostic {
    SyntaxDiagnostic {
        code: diag.code,
        subsystem: diag.subsystem,
        severity: match diag.severity {
            slang::DiagnosticSeverity::Ignored => DiagnosticSeverity::Ignored,
            slang::DiagnosticSeverity::Note => DiagnosticSeverity::Note,
            slang::DiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            slang::DiagnosticSeverity::Error => DiagnosticSeverity::Error,
            slang::DiagnosticSeverity::Fatal => DiagnosticSeverity::Fatal,
        },
        message: diag.message,
        args: diag.args,
        name: diag.name,
        option_name: diag.option_name,
        groups: diag.groups,
        primary_range: diag.primary_range,
        location: diag.location,
        buffer_id: diag.buffer_id,
        file_name: diag.file_name,
    }
}

fn owned_expected_syntax(expected: slang::ParserExpectedSyntax) -> ParserExpectedSyntax {
    ParserExpectedSyntax {
        code: expected.code,
        subsystem: expected.subsystem,
        name: expected.name,
        token_kind: owned_token_kind(expected.token_kind),
        keyword_context: expected.keyword_context.map(owned_keyword_context),
        location: expected.location,
    }
}

fn owned_lexed_token(token: slang::LexedTokenAtOffset) -> LexedTokenAtOffset {
    LexedTokenAtOffset {
        replacement: token.replacement,
        prefix: token.prefix,
        token_kind: owned_token_kind(token.token_kind),
        directive_kind: token.directive_kind.map(owned_node_kind),
    }
}

fn owned_preprocessor_directive(directive: slang::PreprocessorDirective) -> PreprocessorDirective {
    PreprocessorDirective {
        kind: owned_node_kind(directive.kind),
        range: directive.range,
        directive: directive.directive.map(owned_preprocessor_token),
        name: directive.name.map(owned_preprocessor_token),
        include_file_name: directive.include_file_name.map(owned_preprocessor_token),
        params: directive.params.into_iter().map(owned_preprocessor_macro_param).collect(),
        body_tokens: directive.body_tokens.into_iter().map(owned_preprocessor_token).collect(),
        expr_tokens: directive.expr_tokens.into_iter().map(owned_preprocessor_token).collect(),
        disabled_ranges: directive.disabled_ranges,
    }
}

fn owned_directive_trivia_from_preprocessor(
    directive: slang::PreprocessorDirective,
) -> Option<OwnedDirectiveTrivia> {
    Some(OwnedDirectiveTrivia {
        kind: owned_node_kind(directive.kind),
        range: directive.range.and_then(text_range),
        first_token_trivia: Vec::new(),
    })
}

fn owned_preprocessor_token(
    token: slang::PreprocessorDirectiveToken,
) -> PreprocessorDirectiveToken {
    PreprocessorDirectiveToken {
        raw_text: token.raw_text,
        value_text: token.value_text,
        range: token.range,
    }
}

fn owned_preprocessor_macro_param(param: slang::PreprocessorMacroParam) -> PreprocessorMacroParam {
    PreprocessorMacroParam {
        name: param.name.map(owned_preprocessor_token),
        default_tokens: param
            .default_tokens
            .map(|tokens| tokens.into_iter().map(owned_preprocessor_token).collect()),
        range: param.range,
    }
}

fn owned_buffer_ids(ids: slang::SyntaxTreeBufferIds) -> SyntaxTreeBufferIds {
    SyntaxTreeBufferIds {
        root_buffer_id: ids.root_buffer_id,
        source_buffers: ids
            .source_buffers
            .into_iter()
            .map(|buffer| SourceBufferId { path: buffer.path, buffer_id: buffer.buffer_id })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use preproc::{PREPROC_TRACE_CAPABILITY, TraceUnavailableReason};
    use syntax::{
        SyntaxTreeOptions,
        ast::{self, AstNode as _},
        raw::{AstNode, SourceFile, SyntaxElement, SyntaxNode},
    };

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
    fn owned_tree_preserves_ast_slot_shape() {
        let tree = parse_syntax_with_options(
            "module top(input logic clk); logic data; assign data = 1'b0; endmodule\n",
            "source",
            "",
            &SyntaxTreeOptions::without_include_expansion(),
        )
        .unwrap();
        let root = tree.root().unwrap();
        let unit = ast::CompilationUnit::cast(root).unwrap();
        let module =
            unit.members().children().find_map(ast::Member::as_module_declaration).unwrap();
        let header = module.header();
        let ports = header.ports().unwrap().as_ansi_port_list().unwrap();

        assert_eq!(header.name().unwrap().value_text(), "top");
        assert_eq!(ports.ports().children().count(), 1);
        assert!(module.members().children().any(|member| {
            matches!(member, ast::Member::DataDeclaration(_) | ast::Member::ContinuousAssign(_))
        }));
    }

    #[test]
    fn owned_preproc_index_extraction_boundary_reports_macro_and_include() {
        let text = "`define WIDTH 8\n`include \"defs.svh\"\n`WIDTH\n";
        let index =
            preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion());

        assert_eq!(index.defines[0].name.as_deref(), Some("WIDTH"));
        assert_eq!(
            index.includes[0].target,
            preproc::directive_index::MacroIncludeTarget::Literal {
                path: "defs.svh".into(),
                raw: "\"defs.svh\"".into(),
            }
        );
        assert_eq!(index.usages[0].name.as_deref(), Some("WIDTH"));
    }

    #[test]
    fn owned_preproc_index_reports_conditionals_and_predefined_include_branch() {
        let text = "`ifdef USE_A\n`include \"a.svh\"\n`else\n`include \"b.svh\"\n`endif\n";

        let without_define =
            preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion());
        assert_eq!(
            without_define
                .conditionals
                .iter()
                .map(|conditional| conditional.kind)
                .collect::<Vec<_>>(),
            vec![
                preproc::directive_index::MacroConditionalKind::IfDef,
                preproc::directive_index::MacroConditionalKind::Else,
                preproc::directive_index::MacroConditionalKind::EndIf,
            ]
        );
        assert_eq!(
            without_define.includes[0].target,
            preproc::directive_index::MacroIncludeTarget::Literal {
                path: "b.svh".into(),
                raw: "\"b.svh\"".into(),
            }
        );

        let mut options = SyntaxTreeOptions::without_include_expansion();
        options.predefines.push("USE_A".to_owned());
        let with_define = preproc_file_index_from_text(text, &options);
        assert_eq!(
            with_define.includes[0].target,
            preproc::directive_index::MacroIncludeTarget::Literal {
                path: "a.svh".into(),
                raw: "\"a.svh\"".into(),
            }
        );
    }

    #[test]
    fn owned_preproc_index_keeps_token_includes_structural() {
        let text = "`define HEADER \"defs.svh\"\n`include `HEADER\n`include\n`NEXT\n";
        let index =
            preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion());

        assert_eq!(
            index.includes[0].target,
            preproc::directive_index::MacroIncludeTarget::Token { raw: "\"defs.svh\"".into() }
        );
        assert_eq!(
            index.includes[1].target,
            preproc::directive_index::MacroIncludeTarget::Token { raw: "".into() }
        );
    }

    #[test]
    fn owned_preproc_index_reports_inactive_ranges() {
        let text = "`ifdef USE_A\nlogic active;\n`else\nlogic inactive;\n`endif\n";
        let index =
            preproc_file_index_from_text(text, &SyntaxTreeOptions::without_include_expansion());
        let inactive_start = TextSize::from(text.find("logic active;").unwrap() as u32);

        assert!(
            index.inactive_ranges.iter().any(|range| range.contains(inactive_start)),
            "inactive ranges: {:?}",
            index.inactive_ranges
        );
    }

    #[test]
    fn owned_directive_trivia_covers_directive_body() {
        let text = "`define FOO 1\nmodule m; endmodule\n";
        let tree =
            parse_syntax_with_options(text, "source", "", &SyntaxTreeOptions::default()).unwrap();
        let root = tree.root().unwrap();
        let mut directive = None;
        for event in root.elem_preorder() {
            let syntax::WalkEvent::Enter(syntax::SyntaxElement::Token(token)) = event else {
                continue;
            };
            for trivia in token.tok.trivias() {
                if trivia.kind() == syntax::Trivia!["`"] {
                    directive = Some(trivia);
                }
            }
        }
        let directive = directive.unwrap();

        assert_eq!(directive.directive_range().unwrap().start(), TextSize::new(8));
        assert_eq!(directive.directive_range().unwrap().end(), TextSize::new(12));
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
