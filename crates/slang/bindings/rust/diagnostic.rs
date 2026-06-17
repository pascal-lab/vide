use std::ops::{Not, Range};

use crate::{SyntaxKeywordContext, SyntaxKind, TokenKind, ffi};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Ignored,
    Note,
    Warning,
    Error,
    Fatal,
}

impl DiagnosticSeverity {
    const VALUES: [Self; 5] = [Self::Ignored, Self::Note, Self::Warning, Self::Error, Self::Fatal];

    #[inline]
    pub(crate) fn from_raw(value: u8) -> Self {
        Self::VALUES.get(value as usize).copied().unwrap_or(Self::Fatal)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
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
    pub directive_kind: Option<SyntaxKind>,
}

impl ParserExpectedSyntax {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawExpectedSyntax) -> Self {
        Self {
            code: raw.code,
            subsystem: raw.subsystem,
            name: raw.name,
            token_kind: TokenKind::from_id(raw.token_kind),
            keyword_context: raw
                .has_keyword_context
                .then_some(raw.keyword_context)
                .and_then(SyntaxKeywordContext::from_raw),
            location: raw.has_location.then_some(raw.location),
        }
    }
}

impl LexedTokenAtOffset {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawLexedTokenAtOffset) -> Option<Self> {
        raw.has_token.then(|| Self {
            replacement: raw.replacement_start..raw.replacement_end,
            prefix: raw.prefix,
            token_kind: TokenKind::from_id(raw.token_kind),
            directive_kind: raw.has_directive_kind.then(|| SyntaxKind::from_id(raw.directive_kind)),
        })
    }
}

impl SyntaxDiagnostic {
    #[inline]
    pub(crate) fn from_raw(raw: ffi::RawSyntaxDiagnostic) -> Self {
        Self {
            code: raw.code,
            subsystem: raw.subsystem,
            severity: DiagnosticSeverity::from_raw(raw.severity),
            message: raw.message,
            args: raw.args,
            name: raw.name,
            option_name: raw.option_name.is_empty().not().then_some(raw.option_name),
            groups: raw.groups,
            primary_range: raw
                .has_primary_range
                .then_some(raw.primary_range_start..raw.primary_range_end),
            location: raw.has_location.then_some(raw.location),
            buffer_id: raw.has_buffer_id.then_some(raw.buffer_id),
            file_name: raw.file_name.is_empty().not().then_some(raw.file_name),
        }
    }
}
