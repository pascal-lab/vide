use std::{
    fmt,
    sync::{Arc, OnceLock},
};

use cxx::SharedPtr;
use tracing::warn;

use super::{
    ffi,
    syntax_node::{SyntaxNode, SyntaxToken},
};
use crate::{
    diagnostic::{
        DiagCode, LexedTokenAtOffset, ParserExpectedSyntax, SyntaxDiagnostic, SyntaxKeywordContext,
        ffi as diagnostic_ffi,
    },
    source_buffer::SyntaxTreeBufferIds,
};

/// An owned Slang syntax tree.
/// The tree owns the memory for nodes and tokens. Any `SyntaxNode` or
/// `SyntaxToken` borrowed from it must not outlive this value.
#[derive(Clone)]
pub struct SyntaxTree {
    pub(crate) raw: SharedPtr<ffi::SyntaxTree>,
    preprocessor_trace_cache: Arc<OnceLock<crate::preproc::Trace>>,
}

#[derive(Debug, Clone)]
pub struct SyntaxTreeWithTrace {
    pub tree: SyntaxTree,
    pub preprocessor_trace: crate::preproc::Trace,
}

/// Parser options for creating a syntax tree.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTreeOptions {
    pub predefines: Vec<String>,
    pub include_paths: Vec<String>,
    pub include_buffers: Vec<SyntaxTreeBuffer>,
    pub expand_includes: bool,
    pub collect_expected_syntax: bool,
    /// Restrict parser expectation collection to one cursor offset. This keeps
    /// completion metadata local to a single query instead of attaching every
    /// recovery window to an authoritative syntax tree.
    pub expected_syntax_offset: Option<usize>,
}

/// In-memory source buffer that can be used for include resolution.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxTreeBuffer {
    pub path: String,
    pub text: String,
}

impl Default for SyntaxTreeOptions {
    fn default() -> Self {
        Self {
            predefines: Vec::new(),
            include_paths: Vec::new(),
            include_buffers: Vec::new(),
            expand_includes: true,
            collect_expected_syntax: false,
            expected_syntax_offset: None,
        }
    }
}

impl SyntaxTreeOptions {
    pub fn without_include_expansion() -> Self {
        Self { expand_includes: false, ..Self::default() }
    }
}

impl SyntaxTree {
    pub(crate) fn from_raw(raw: SharedPtr<ffi::SyntaxTree>) -> Self {
        Self { raw, preprocessor_trace_cache: Arc::new(OnceLock::new()) }
    }

    pub fn from_text(text: &str, name: &str, path: &str) -> Self {
        Self::from_text_with_options(text, name, path, &SyntaxTreeOptions::default())
    }

    pub fn from_text_with_options(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> Self {
        Self::parse_with_options(text, name, path, options, true)
    }

    /// Parse source using an explicit Slang root mode.
    fn parse_with_options(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
        guess: bool,
    ) -> Self {
        Self::from_raw(ffi::parse_syntax_tree(
            text,
            name,
            path,
            options.predefines.clone(),
            options.include_paths.clone(),
            options.include_buffers.iter().map(|buffer| buffer.path.clone()).collect(),
            options.include_buffers.iter().map(|buffer| buffer.text.clone()).collect(),
            options.expand_includes,
            guess,
            options.collect_expected_syntax,
            options.expected_syntax_offset.unwrap_or_default(),
            options.expected_syntax_offset.is_some(),
        ))
    }

    /// Parse source as a compilation unit without constructing a guessed root.
    pub fn from_file_in_memory(text: &str, name: &str, path: &str) -> Self {
        Self::from_file_in_memory_with_options(text, name, path, &SyntaxTreeOptions::default())
    }

    /// Parse source as a compilation unit, preserving the file-root contract
    /// required by HIR and semantic indexing.
    pub fn from_file_in_memory_with_options(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> Self {
        Self::parse_with_options(text, name, path, options, false)
    }

    /// Parse source as a compilation unit, preserving the file-root contract
    /// required by HIR and semantic indexing.
    pub fn from_file_in_memory_with_options_and_trace(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTreeWithTrace {
        let tree = Self::from_file_in_memory_with_options(text, name, path, options);
        let preprocessor_trace = tree.build_preprocessor_trace();
        SyntaxTreeWithTrace { tree, preprocessor_trace }
    }

    pub fn from_text_with_options_and_trace(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTreeWithTrace {
        let tree = Self::from_text_with_options(text, name, path, options);
        let preprocessor_trace = tree.build_preprocessor_trace();
        SyntaxTreeWithTrace { tree, preprocessor_trace }
    }

    pub fn from_library_map_text(text: &str, name: &str, path: &str) -> Self {
        Self::from_raw(ffi::parse_library_map_syntax_tree(text, name, path, false, 0, false))
    }

    pub fn root(&self) -> SyntaxNode<'_> {
        let raw =
            ffi::syntax_tree_root(self.raw.as_ref().expect("Slang returned a null syntax tree"));
        SyntaxNode::from_nullable_raw(raw, self).expect("Slang returned null syntax tree root")
    }

    /// NOTE: This will only get diagnostics while parsing. For further
    /// diagnostics provided by Slang, use Compilation structure.
    pub fn diagnostics(&self, warning_options: &[String]) -> Vec<SyntaxDiagnostic> {
        let raw = self.raw.as_ref().expect("Slang returned a null syntax tree");
        diagnostic_ffi::syntax_tree_diagnostics(raw, warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
    }

    pub fn diagnostics_with_options(&self, warning_options: &[String]) -> Vec<SyntaxDiagnostic> {
        self.diagnostics(warning_options)
    }

    pub fn expected_syntax_at_offset(
        text: &str,
        name: &str,
        path: &str,
        offset: usize,
    ) -> Vec<ParserExpectedSyntax> {
        Self::expected_syntax_at_offset_with_options(
            text,
            name,
            path,
            offset,
            &SyntaxTreeOptions::default(),
        )
    }

    pub fn expected_syntax_at_offset_with_options(
        text: &str,
        name: &str,
        path: &str,
        offset: usize,
        options: &SyntaxTreeOptions,
    ) -> Vec<ParserExpectedSyntax> {
        let mut options = options.clone();
        options.collect_expected_syntax = false;
        options.expected_syntax_offset = Some(offset);
        let tree = Self::from_file_in_memory_with_options(text, name, path, &options);
        tree.expected_syntax_at(offset)
    }

    pub fn library_map_expected_syntax_at_offset(
        text: &str,
        name: &str,
        path: &str,
        offset: usize,
    ) -> Vec<ParserExpectedSyntax> {
        let tree = Self::from_raw(ffi::parse_library_map_syntax_tree(
            text, name, path, false, offset, true,
        ));
        tree.expected_syntax_at(offset)
    }

    pub fn directive_at_offset(
        text: &str,
        _name: &str,
        _path: &str,
        offset: usize,
    ) -> Option<LexedTokenAtOffset> {
        let (start, end, name) = word_after_backtick_at_offset(text, offset)?;
        Some(LexedTokenAtOffset {
            replacement: start..end,
            prefix: text.get(start..offset)?.to_owned(),
            token_kind: crate::token::TokenKind::DIRECTIVE,
            directive_kind: (!name.is_empty()).then(|| SyntaxToken::directive_kind(name)),
        })
    }

    pub fn token_word_at_offset(
        text: &str,
        _name: &str,
        _path: &str,
        offset: usize,
    ) -> Option<LexedTokenAtOffset> {
        let (start, end, system) = identifier_at_offset(text, offset)?;
        Some(LexedTokenAtOffset {
            replacement: start..end,
            prefix: text.get(start..offset)?.to_owned(),
            token_kind: if system {
                crate::token::TokenKind::SYSTEM_IDENTIFIER
            } else {
                crate::token::TokenKind::IDENTIFIER
            },
            directive_kind: None,
        })
    }

    pub fn expected_syntax_at(&self, offset: usize) -> Vec<ParserExpectedSyntax> {
        let raw = self.raw.as_ref().expect("Slang returned a null syntax tree");
        ffi::syntax_tree_expected_syntax_at(raw, offset)
            .into_iter()
            .map(ParserExpectedSyntax::from_raw)
            .collect()
    }

    pub fn preprocessor_trace(&self) -> crate::preproc::Trace {
        self.preprocessor_trace_cache
            .get_or_init(|| {
                crate::preproc::Trace::from_raw(ffi::syntax_tree_preprocessor_trace(
                    self.raw.as_ref().expect("Slang returned a null syntax tree"),
                ))
            })
            .clone()
    }

    fn build_preprocessor_trace(&self) -> crate::preproc::Trace {
        self.preprocessor_trace()
    }

    pub fn buffer_id(&self) -> u32 {
        ffi::syntax_tree_root_buffer_id(self.raw.as_ref().expect("null Slang syntax tree"))
    }

    pub fn buffer_ids(&self) -> SyntaxTreeBufferIds {
        let trace = self.preprocessor_trace();
        SyntaxTreeBufferIds {
            root_buffer_id: trace.root_buffer_id,
            source_buffers: trace.source_buffers,
        }
    }
}

impl ParserExpectedSyntax {
    pub fn diagnostic_code(&self) -> DiagCode {
        DiagCode::from_raw(self.subsystem, self.code)
    }

    fn from_raw(raw: ffi::RawExpectedSyntax) -> Self {
        let code = DiagCode::from_raw(raw.subsystem, raw.code);
        let name = code.info().map_or_else(
            || {
                warn!(
                    subsystem = raw.subsystem,
                    code = raw.code,
                    "Slang returned an unknown expected syntax diagnostic"
                );
                format!("UnknownDiagnostic({}:{})", raw.subsystem, raw.code)
            },
            |info| info.name.to_owned(),
        );
        Self {
            code: raw.code,
            subsystem: raw.subsystem,
            name,
            token_kind: crate::token::TokenKind::from_raw(raw.token_kind),
            keyword_context: raw
                .has_keyword_context
                .then(|| {
                    SyntaxKeywordContext::from_raw(raw.keyword_context).or_else(|| {
                    warn!(
                        context = raw.keyword_context,
                        "Slang returned an unknown expected syntax context; dropping the context"
                    );
                    None
                })
                })
                .flatten(),
            location: raw.has_location.then_some(raw.location),
            end: raw.has_end.then_some(raw.end),
        }
    }
}

fn word_after_backtick_at_offset(text: &str, offset: usize) -> Option<(usize, usize, &str)> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let bytes = text.as_bytes();
    let mut index = 0;
    let mut state = LexState::Code;
    while index < bytes.len() {
        match state {
            LexState::Code => match bytes[index] {
                b'"' => {
                    state = LexState::String;
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'/') => {
                    state = LexState::LineComment;
                    index += 2;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    state = LexState::BlockComment;
                    index += 2;
                }
                b'`' => {
                    let start = index + 1;
                    let mut end = start;
                    while bytes.get(end).is_some_and(|byte| is_identifier_continue(*byte)) {
                        end += 1;
                    }
                    if start <= offset && offset <= end {
                        return Some((start, end, text.get(start..end)?));
                    }
                    index = end.max(index + 1);
                }
                _ => index += 1,
            },
            LexState::String => {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == b'"' {
                    state = LexState::Code;
                    index += 1;
                } else {
                    index += 1;
                }
            }
            LexState::LineComment => {
                if bytes[index] == b'\n' || bytes[index] == b'\r' {
                    state = LexState::Code;
                }
                index += 1;
            }
            LexState::BlockComment => {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    state = LexState::Code;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        }
    }
    None
}

fn identifier_at_offset(text: &str, offset: usize) -> Option<(usize, usize, bool)> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return None;
    }

    let bytes = text.as_bytes();
    let mut index = 0;
    let mut state = LexState::Code;
    while index < bytes.len() {
        match state {
            LexState::Code => match bytes[index] {
                b'"' => {
                    state = LexState::String;
                    index += 1;
                }
                b'/' if bytes.get(index + 1) == Some(&b'/') => {
                    state = LexState::LineComment;
                    index += 2;
                }
                b'/' if bytes.get(index + 1) == Some(&b'*') => {
                    state = LexState::BlockComment;
                    index += 2;
                }
                byte if is_identifier_start(byte) => {
                    let start = index;
                    let system = byte == b'$';
                    index += 1;
                    while bytes.get(index).is_some_and(|byte| is_identifier_continue(*byte)) {
                        index += 1;
                    }
                    let end = index;
                    let at_end = offset == end
                        && bytes.get(offset).is_some_and(|byte| is_identifier_start(*byte));
                    if start <= offset && offset <= end && !at_end {
                        return Some((start, end, system));
                    }
                }
                _ => index += 1,
            },
            LexState::String => {
                if bytes[index] == b'\\' {
                    index = (index + 2).min(bytes.len());
                } else if bytes[index] == b'"' {
                    state = LexState::Code;
                    index += 1;
                } else {
                    index += 1;
                }
            }
            LexState::LineComment => {
                if bytes[index] == b'\n' || bytes[index] == b'\r' {
                    state = LexState::Code;
                }
                index += 1;
            }
            LexState::BlockComment => {
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    state = LexState::Code;
                    index += 2;
                } else {
                    index += 1;
                }
            }
        }
    }
    None
}

fn is_identifier_start(byte: u8) -> bool {
    matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'_') || byte == b'$'
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

#[derive(Clone, Copy)]
enum LexState {
    Code,
    String,
    LineComment,
    BlockComment,
}

// SAFETY: `SyntaxTree` owns the Slang syntax tree through a `cxx::SharedPtr`.
// Slang keeps the source manager and syntax nodes alive behind that owner, and
// every Rust view borrows this tree. The wrapper exposes only const access to
// the tree after parsing; the mutable C++ calls are confined to construction
// and trace collection before the tree is shared with callers.
unsafe impl Send for SyntaxTree {}
unsafe impl Sync for SyntaxTree {}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxTree").finish()
    }
}

impl PartialEq for SyntaxTree {
    fn eq(&self, other: &Self) -> bool {
        self.root() == other.root()
    }
}

impl Eq for SyntaxTree {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        syntax::SyntaxKind,
        token::{TokenKind, TriviaKind},
    };

    #[test]
    fn parser_metadata_is_authoritative() {
        let text = "module demo; initial begin <caret> end endmodule";
        let offset = text.find("<caret>").unwrap();
        let text = text.replace("<caret>", "");
        let expected = SyntaxTree::expected_syntax_at_offset(&text, "source", "", offset);
        assert!(expected.iter().any(|item| item.name == "ExpectedStatement"));
    }

    #[test]
    fn inline_trivia_is_read_without_borrowing_a_temporary_view() {
        let tree =
            SyntaxTree::from_file_in_memory("module m; wire a; endmodule", "source", "source.sv");
        let trivia = tree
            .root()
            .tokens()
            .flat_map(|token| token.tok.trivias())
            .find(|trivia| trivia.get_raw_text() == " ")
            .expect("source should contain a whitespace trivia");

        assert_eq!(trivia.kind(), TriviaKind::WHITESPACE);
        assert_eq!(trivia.get_raw_text(), " ");
        assert_eq!(trivia.kind(), TriviaKind::WHITESPACE);
    }

    #[test]
    fn parser_metadata_survives_named_source() {
        let options =
            SyntaxTreeOptions { collect_expected_syntax: true, ..SyntaxTreeOptions::default() };
        let tree = SyntaxTree::from_text_with_options(
            "module demo; initial begin  end endmodule",
            "source",
            "source.sv",
            &options,
        );
        let expected = tree.expected_syntax_at(28);
        assert!(expected.iter().any(|item| item.name == "ExpectedStatement"));
    }

    #[test]
    fn record_all_preserves_overlapping_expectation_windows() {
        let text = "module m; always begin begin end endmodule";
        let options =
            SyntaxTreeOptions { collect_expected_syntax: true, ..SyntaxTreeOptions::default() };
        let tree =
            SyntaxTree::from_file_in_memory_with_options(text, "source", "source.sv", &options);
        let expected = tree.expected_syntax_at(28);
        assert_eq!(expected.len(), 2);
        assert!(expected.iter().all(|item| item.name == "ExpectedStatement"));
        assert_eq!(expected[0].location, Some(22));
        assert_eq!(expected[0].end, Some(28));
        assert_eq!(expected[1].location, Some(28));
        assert_eq!(expected[1].end, Some(32));
    }

    #[test]
    fn inspect_completion_fixture_expectations() {
        for (text, _offset) in [
            ("module m; endmodule\n", 21),
            ("module m(input a,\n  \n); endmodule\n", 20),
            ("module m; initial f(); endmodule", 20),
        ] {
            let values: Vec<_> = (0..=text.len())
                .map(|offset| {
                    (offset, SyntaxTree::expected_syntax_at_offset(text, "test", "test.v", offset))
                })
                .filter(|(_, values)| !values.is_empty())
                .collect();
            eprintln!("{text:?} -> {values:?}");
        }
    }

    #[test]
    fn directive_at_offset_uses_source_boundaries() {
        let text = "`define\nmodule m; endmodule\n";
        let directive = SyntaxTree::directive_at_offset(text, "source", "", 3)
            .expect("directive word should be lexed at its cursor");
        assert_eq!(directive.replacement, 1..7);
        assert_eq!(directive.prefix, "de");
        assert_eq!(directive.token_kind, TokenKind::DIRECTIVE);
        assert_eq!(directive.directive_kind, Some(SyntaxKind::DEFINE_DIRECTIVE));
    }

    #[test]
    fn directive_at_offset_ignores_strings_and_comments() {
        for text in ["\"`define\"", "// `define\nmodule m; endmodule\n"] {
            assert_eq!(SyntaxTree::directive_at_offset(text, "source", "", 3), None);
        }
    }

    #[test]
    fn token_word_at_offset_reports_identifier() {
        let word = SyntaxTree::token_word_at_offset("library/*cursor*/", "source", "", 7)
            .expect("identifier should be available at its cursor");
        assert_eq!(word.replacement, 0..7);
        assert_eq!(word.prefix, "library");
        assert_eq!(word.token_kind, TokenKind::IDENTIFIER);
    }

    #[test]
    fn token_word_at_offset_ignores_non_code() {
        assert_eq!(SyntaxTree::token_word_at_offset("4 2", "source", "", 1), None);
        assert_eq!(SyntaxTree::token_word_at_offset("\"value\"", "source", "", 4), None);
    }
}
