use std::ops::Range;

use preproc::DirectiveKind;
use syntax::TokenKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseOptions {
    pub predefines: Vec<String>,
    pub include_paths: Vec<String>,
    pub include_buffers: Vec<SourceBuffer>,
    pub expand_includes: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            predefines: Vec::new(),
            include_paths: Vec::new(),
            include_buffers: Vec::new(),
            expand_includes: true,
        }
    }
}

impl ParseOptions {
    pub fn without_include_expansion() -> Self {
        Self { expand_includes: false, ..Self::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBuffer {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBufferIds {
    pub root_buffer_id: u32,
    pub source_buffers: Vec<SourceBufferId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBufferId {
    pub path: String,
    pub buffer_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontendDiagnostic {
    pub code: u16,
    pub subsystem: u16,
    pub severity: DiagnosticSeverity,
    pub message: String,
    pub args: Vec<String>,
    pub name: String,
    pub option_name: Option<String>,
    pub groups: Vec<String>,
    pub primary_range: Option<Range<usize>>,
    pub location: Option<usize>,
    pub buffer_id: Option<u32>,
    pub file_name: Option<String>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Ignored,
    Note,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserExpectedSyntax {
    pub code: u16,
    pub subsystem: u16,
    pub name: String,
    pub token_kind: TokenKind,
    pub keyword_context: Option<SyntaxKeywordContext>,
    pub location: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexedTokenAtOffset {
    pub replacement: Range<usize>,
    pub prefix: String,
    pub token_kind: TokenKind,
    pub directive_kind: Option<DirectiveKind>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxKeywordContext {
    CompilationUnitMember,
    LibraryMapMember,
    ModuleHeaderItem,
    ModuleMember,
    GenerateMember,
    SpecifyItem,
    ConfigHeaderItem,
    ConfigRule,
    BlockItem,
    Statement,
    ParameterPortListItem,
    AnsiPortItem,
    FunctionPortItem,
    GateType,
}

pub struct SyntaxFacts;
pub struct SemanticFacts;

impl SyntaxFacts {
    pub fn keyword_candidates_for_context(
        _version: &str,
        context: SyntaxKeywordContext,
    ) -> Vec<String> {
        keyword_context_candidates(context).iter().map(|keyword| (*keyword).to_owned()).collect()
    }
}

impl SemanticFacts {
    pub fn is_edge_kind(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::POS_EDGE_KEYWORD | TokenKind::NEG_EDGE_KEYWORD | TokenKind::EDGE_KEYWORD
        )
    }
}

fn keyword_context_candidates(context: SyntaxKeywordContext) -> &'static [&'static str] {
    match context {
        SyntaxKeywordContext::CompilationUnitMember => &[
            "bind",
            "checker",
            "config",
            "interface",
            "macromodule",
            "module",
            "package",
            "primitive",
            "program",
        ],
        SyntaxKeywordContext::LibraryMapMember => &["config", "include", "library"],
        SyntaxKeywordContext::ModuleHeaderItem => {
            &["input", "inout", "localparam", "output", "parameter", "ref"]
        }
        SyntaxKeywordContext::ModuleMember => &[
            "alias",
            "always",
            "always_comb",
            "always_ff",
            "always_latch",
            "and",
            "assign",
            "begin",
            "buf",
            "bufif0",
            "bufif1",
            "case",
            "function",
            "generate",
            "genvar",
            "if",
            "initial",
            "input",
            "localparam",
            "logic",
            "output",
            "parameter",
            "task",
            "typedef",
            "wire",
        ],
        SyntaxKeywordContext::GenerateMember => &[
            "and",
            "assign",
            "begin",
            "buf",
            "bufif0",
            "bufif1",
            "case",
            "for",
            "if",
            "localparam",
            "parameter",
            "typedef",
            "wire",
        ],
        SyntaxKeywordContext::SpecifyItem => &[
            "if",
            "ifnone",
            "pulsestyle_ondetect",
            "pulsestyle_onevent",
            "showcancelled",
            "specparam",
        ],
        SyntaxKeywordContext::ConfigHeaderItem => &["cell", "design", "instance", "localparam"],
        SyntaxKeywordContext::ConfigRule => &["cell", "default", "instance"],
        SyntaxKeywordContext::BlockItem => &[
            "automatic",
            "begin",
            "case",
            "for",
            "foreach",
            "if",
            "integer",
            "localparam",
            "parameter",
            "return",
            "while",
        ],
        SyntaxKeywordContext::Statement => &[
            "begin", "case", "do", "for", "forever", "foreach", "if", "repeat", "return", "wait",
            "while",
        ],
        SyntaxKeywordContext::ParameterPortListItem => &["localparam", "parameter", "type"],
        SyntaxKeywordContext::AnsiPortItem => &["input", "inout", "logic", "output", "ref", "wire"],
        SyntaxKeywordContext::FunctionPortItem => &["input", "inout", "output", "ref"],
        SyntaxKeywordContext::GateType => &[
            "and", "buf", "bufif0", "bufif1", "cmos", "nand", "nor", "not", "notif0", "notif1",
            "or", "rcmos", "rnmos", "rpmos", "xnor", "xor",
        ],
    }
}
