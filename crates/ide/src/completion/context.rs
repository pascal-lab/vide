//! Completion context detection.

mod caret;
mod decl_name;
mod expected;
mod lex;
mod parser;
mod resolve;
mod util;

use hir::base_db::source_db::{SourceDb, SourceRootDb};
use semantics::Semantics;
use smallvec::{SmallVec, smallvec};
use syntax::{
    ParserExpectedSyntax, SyntaxKeywordContext, SyntaxNode, SyntaxNodeExt,
    has_text_range::HasTextRange,
};
use utils::line_index::{TextRange, TextSize};

use self::caret::CaretSnapshot;
use crate::{FilePosition, db::root_db::RootDb};

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
    Dollar,
    Backtick,
    Apostrophe,
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
    IntegerLiteralBase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpectationSource {
    Parser,
    DeclarationName,
    Ast(syntax::SyntaxKind),
    Token(syntax::TokenKind),
    Trigger(TriggerChar),
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

struct CompletionWord {
    replacement: TextRange,
    prefix: String,
}

pub(crate) fn completion_context(
    db: &RootDb,
    FilePosition { file_id, offset }: FilePosition,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    let sema = Semantics::new(db);
    let parsed_file = sema.parse_file(file_id);
    let Some(root) = parsed_file.root() else {
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
    let directive_word = directive_word_at_offset(&text, offset);
    let token_word = library_map_word_at_offset(root, &text, offset);
    let system_word = standalone_system_identifier_word_at_offset(&text, offset);
    detect_completion_context_impl(
        root,
        offset,
        trigger,
        directive_word,
        token_word,
        system_word,
        Some(&parser_expected_syntax),
    )
}

pub fn detect_completion_context(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
) -> CompletionContext {
    detect_completion_context_impl(root, offset, trigger, None, None, None, None)
}

pub fn detect_completion_context_with_source_text(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    source_text: &str,
) -> CompletionContext {
    let parser_expected_syntax = parser_expected_syntax_for_text(root, source_text, offset);
    let directive_word = directive_word_at_offset(source_text, offset);
    let token_word = library_map_word_at_offset(root, source_text, offset);
    let system_word = standalone_system_identifier_word_at_offset(source_text, offset);
    detect_completion_context_impl(
        root,
        offset,
        trigger,
        directive_word,
        token_word,
        system_word,
        Some(&parser_expected_syntax),
    )
}

fn detect_completion_context_impl(
    root: SyntaxNode<'_>,
    offset: TextSize,
    trigger: Option<TriggerChar>,
    directive_word: Option<CompletionWord>,
    token_word: Option<CompletionWord>,
    system_word: Option<CompletionWord>,
    parser_expected_syntax: Option<&[ParserExpectedSyntax]>,
) -> CompletionContext {
    let caret = CaretSnapshot::new(root, offset);
    let (mut replacement, mut prefix) = caret.replacement_and_prefix();

    let lex = lex::detect_lex_context(&caret);
    if matches!(lex, LexContext::Code | LexContext::Literal)
        && let Some(word) = integer_literal_base_word_at_offset(&caret, offset)
    {
        replacement = word.replacement;
        prefix = word.prefix;
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            expectations: smallvec![CompletionExpectation {
                syntax: ExpectedSyntax::IntegerLiteralBase,
                source: ExpectationSource::Token(syntax::Token!["'"]),
            }],
            in_decl_name: false,
        };
    }

    if lex != LexContext::Code {
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            expectations: SmallVec::new(),
            in_decl_name: false,
        };
    }

    if let Some(word) = directive_word {
        replacement = word.replacement;
        prefix = word.prefix;
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            expectations: smallvec![CompletionExpectation {
                syntax: ExpectedSyntax::DirectiveName,
                source: ExpectationSource::Token(syntax::TokenKind::DIRECTIVE),
            }],
            in_decl_name: false,
        };
    }

    if prefix.is_empty()
        && let Some(word) = token_word.filter(|word| !word.prefix.is_empty())
    {
        replacement = word.replacement;
        prefix = word.prefix;
    }

    if prefix.is_empty()
        && let Some(word) = system_word
    {
        replacement = word.replacement;
        prefix = word.prefix;
    }

    if trigger == Some(TriggerChar::Backtick) {
        return CompletionContext {
            replacement,
            prefix,
            trigger,
            lex,
            expectations: smallvec![CompletionExpectation {
                syntax: ExpectedSyntax::DirectiveName,
                source: ExpectationSource::Trigger(TriggerChar::Backtick),
            }],
            in_decl_name: false,
        };
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

fn directive_word_at_offset(source_text: &str, offset: TextSize) -> Option<CompletionWord> {
    let directive =
        syntax::SyntaxTree::directive_at_offset(source_text, "source", "", usize::from(offset))?;
    Some(CompletionWord {
        replacement: TextRange::new(
            TextSize::from(directive.replacement.start as u32),
            TextSize::from(directive.replacement.end as u32),
        ),
        prefix: directive.prefix,
    })
}

fn library_map_word_at_offset(
    root: SyntaxNode<'_>,
    source_text: &str,
    offset: TextSize,
) -> Option<CompletionWord> {
    if root.kind() != syntax::SyntaxKind::LIBRARY_MAP {
        return None;
    }

    let word =
        syntax::SyntaxTree::token_word_at_offset(source_text, "source", "", usize::from(offset))?;
    Some(CompletionWord {
        replacement: TextRange::new(
            TextSize::from(word.replacement.start as u32),
            TextSize::from(word.replacement.end as u32),
        ),
        prefix: word.prefix,
    })
}

fn standalone_system_identifier_word_at_offset(
    source_text: &str,
    offset: TextSize,
) -> Option<CompletionWord> {
    let offset = usize::from(offset);
    if offset == 0 || offset > source_text.len() || !source_text.is_char_boundary(offset) {
        return None;
    }

    let start = offset - 1;
    if source_text.as_bytes().get(start) != Some(&b'$') {
        return None;
    }

    Some(CompletionWord {
        replacement: TextRange::new(TextSize::from(start as u32), TextSize::from(offset as u32)),
        prefix: "$".to_owned(),
    })
}

fn integer_literal_base_word_at_offset(
    caret: &CaretSnapshot<'_>,
    offset: TextSize,
) -> Option<CompletionWord> {
    // Only recover from token shapes that slang has already produced:
    // <integer> ' and <integer> 's.
    let prev = caret.root.token_before_offset(offset)?;
    let prev_range = prev.text_range()?;
    if prev_range.end() != offset {
        return None;
    }

    if !is_integer_literal_size_before(caret, prev_range.start()) {
        return None;
    }

    match prev.kind() {
        syntax::Token!["'"] => {
            Some(CompletionWord { replacement: TextRange::empty(offset), prefix: String::new() })
        }
        syntax::TokenKind::INTEGER_BASE => {
            let raw = prev.tok.raw_text().to_string();
            if !matches!(raw.as_str(), "'s" | "'S") {
                return None;
            }

            Some(CompletionWord {
                replacement: TextRange::new(prev_range.start() + TextSize::new(1), offset),
                prefix: String::from("s"),
            })
        }
        _ => None,
    }
}

fn is_integer_literal_size_before(caret: &CaretSnapshot<'_>, offset: TextSize) -> bool {
    let Some(prev) = caret.root.token_before_offset(offset) else {
        return false;
    };
    prev.kind() == syntax::TokenKind::INTEGER_LITERAL
        && prev.text_range().is_some_and(|range| range.end() == offset)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        sync::{
            Mutex, OnceLock,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use syntax::SyntaxTree;

    use super::*;

    static PARSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    static NEXT_FILE_ID: AtomicUsize = AtomicUsize::new(0);

    fn fixture_context(fixture: &ContextFixture) -> CompletionContext {
        let _guard = PARSE_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        let marker = "/*caret*/";
        let off = fixture.source.find(marker).expect("missing /*caret*/");
        let owned = fixture.source.replace(marker, "");
        let id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let tree = match fixture.source_kind {
            ContextSourceKind::SystemVerilog => {
                let path = format!("test_{id}.v");
                SyntaxTree::from_text(&owned, "test", &path)
            }
            ContextSourceKind::LibraryMap => {
                SyntaxTree::from_library_map_text(&owned, "test", &format!("test_{id}.map"))
            }
        };

        let root = tree.root().unwrap();
        detect_completion_context_with_source_text(
            root,
            TextSize::from(off as u32),
            fixture.trigger,
            &owned,
        )
    }

    #[derive(Debug, Clone, Copy)]
    enum ContextSourceKind {
        SystemVerilog,
        LibraryMap,
    }

    struct ContextFixture {
        source: String,
        trigger: Option<TriggerChar>,
        source_kind: ContextSourceKind,
    }

    impl ContextFixture {
        fn read(path: &Path) -> Self {
            let raw = fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
            let mut trigger = None;
            let mut final_newline = true;
            let source_kind = match path.extension().and_then(|extension| extension.to_str()) {
                Some("map") => ContextSourceKind::LibraryMap,
                _ => ContextSourceKind::SystemVerilog,
            };
            let mut source = String::new();

            for line in raw.lines() {
                let Some(meta) = line.strip_prefix("//- ") else {
                    source.push_str(line);
                    source.push('\n');
                    continue;
                };

                let (key, value) = meta
                    .split_once(':')
                    .unwrap_or_else(|| panic!("invalid fixture metadata in {}", path.display()));
                match key.trim() {
                    "trigger" => trigger = Some(parse_context_trigger(value.trim(), path)),
                    "final-newline" => {
                        final_newline = value.trim().parse().unwrap_or_else(|_| {
                            panic!("invalid final-newline metadata in {}", path.display())
                        })
                    }
                    other => panic!("unknown fixture metadata key `{other}` in {}", path.display()),
                }
            }

            if !final_newline && source.ends_with('\n') {
                source.pop();
            }

            Self { source, trigger, source_kind }
        }
    }

    fn parse_context_trigger(value: &str, path: &Path) -> TriggerChar {
        match value {
            "dot" => TriggerChar::Dot,
            "open_paren" => TriggerChar::OpenParen,
            "comma" => TriggerChar::Comma,
            "at" => TriggerChar::At,
            "hash" => TriggerChar::Hash,
            "dollar" => TriggerChar::Dollar,
            "backtick" => TriggerChar::Backtick,
            "apostrophe" => TriggerChar::Apostrophe,
            "newline" => TriggerChar::Newline,
            other => panic!("unknown trigger `{other}` in {}", path.display()),
        }
    }

    fn context_snapshot(c: &CompletionContext) -> String {
        let mut out = format!(
            "lex: {:?}\nprefix: {:?}\nreplacement: {:?}\nin_decl_name: {}\nexpectations:",
            c.lex, c.prefix, c.replacement, c.in_decl_name
        );
        if c.expectations.is_empty() {
            out.push_str("\n  <none>");
        } else {
            for expectation in &c.expectations {
                out.push_str(&format!(
                    "\n  {:?} from {:?}",
                    expectation.syntax, expectation.source
                ));
            }
        }
        out
    }

    #[test]
    fn context_fixtures() {
        insta::glob!("context/fixtures/*", |path| {
            let fixture = ContextFixture::read(path);
            let c = fixture_context(&fixture);
            insta::assert_snapshot!(context_snapshot(&c));
        });
    }
}
