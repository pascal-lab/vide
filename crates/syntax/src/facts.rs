use crate::{SyntaxKeywordContext, TokenKind};

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
