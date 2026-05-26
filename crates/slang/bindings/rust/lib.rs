#![feature(trait_alias)]

pub mod ast;
mod ffi;
mod syntax;
mod token;

use std::{
    ffi::c_char,
    fmt::{self, Display},
    hash, iter,
    ops::{Not, Range},
    pin::Pin,
};

use cxx::{SharedPtr, UniquePtr};
pub use ffi::CxxSV;
use itertools::Either;
pub use syntax::{
    SyntaxKind, TokenKind, TriviaKind,
    cursor::SyntaxCursor,
    iter::{
        SyntaxAncestors, SyntaxChildren, SyntaxElemPreorder, SyntaxIdxChildren, SyntaxNodePreorder,
        WalkEvent,
    },
};

pub struct SVInt {
    _ptr: UniquePtr<ffi::SVInt>,
}

pub struct SVLogic {
    _ptr: UniquePtr<ffi::SVLogic>,
}

pub struct SourceLocation {
    _ptr: UniquePtr<ffi::SourceLocation>,
}

pub struct SourceRange {
    _ptr: UniquePtr<ffi::SourceRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxTriviaLoc {
    pub buffer_id: u32,
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Copy)]
pub struct SyntaxNode<'a> {
    _ptr: Pin<&'a ffi::SyntaxNode>,
}

#[derive(Clone, Copy)]
pub struct SyntaxToken<'a> {
    _ptr: Pin<&'a ffi::SyntaxToken>,
}

#[derive(Clone)]
pub struct SyntaxTree {
    _ptr: SharedPtr<ffi::SyntaxTree>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeOptions {
    pub predefines: Vec<String>,
    pub include_paths: Vec<String>,
    pub include_buffers: Vec<SyntaxTreeBuffer>,
    pub expand_includes: bool,
}

impl Default for SyntaxTreeOptions {
    fn default() -> Self {
        Self {
            predefines: Vec::new(),
            include_paths: Vec::new(),
            include_buffers: Vec::new(),
            expand_includes: true,
        }
    }
}

impl SyntaxTreeOptions {
    pub fn without_include_expansion() -> Self {
        Self { expand_includes: false, ..Self::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeBuffer {
    pub path: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxTreeBufferIds {
    pub root_buffer_id: u32,
    pub source_buffers: Vec<SourceBufferId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBufferId {
    pub path: String,
    pub buffer_id: u32,
}

#[derive(Clone, Copy)]
pub struct SyntaxTrivia<'a> {
    _ptr: Pin<&'a ffi::SyntaxTrivia>,
}

pub struct SemanticFacts;
pub struct SyntaxFacts;

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

impl SyntaxKeywordContext {
    const VALUES: [Self; 14] = [
        Self::CompilationUnitMember,
        Self::LibraryMapMember,
        Self::ModuleHeaderItem,
        Self::ModuleMember,
        Self::GenerateMember,
        Self::SpecifyItem,
        Self::ConfigHeaderItem,
        Self::ConfigRule,
        Self::BlockItem,
        Self::Statement,
        Self::ParameterPortListItem,
        Self::AnsiPortItem,
        Self::FunctionPortItem,
        Self::GateType,
    ];

    #[inline]
    fn from_raw(value: u8) -> Option<Self> {
        Self::VALUES.get(value as usize).copied()
    }
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

impl DiagnosticSeverity {
    const VALUES: [Self; 5] = [Self::Ignored, Self::Note, Self::Warning, Self::Error, Self::Fatal];

    #[inline]
    fn from_raw(value: u8) -> Self {
        Self::VALUES.get(value as usize).copied().unwrap_or(Self::Fatal)
    }
}

impl SyntaxDiagnostic {
    #[inline]
    fn from_raw(raw: ffi::RawSyntaxDiagnostic) -> Self {
        Self {
            code: raw.code,
            subsystem: raw.subsystem,
            severity: DiagnosticSeverity::from_raw(raw.severity),
            message: raw.message,
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

impl SyntaxTreeBufferIds {
    #[inline]
    fn from_raw(raw: ffi::RawSyntaxTreeBufferIds) -> Self {
        Self {
            root_buffer_id: raw.root_buffer_id,
            source_buffers: raw
                .source_buffers
                .into_iter()
                .map(|buffer| SourceBufferId { path: buffer.path, buffer_id: buffer.buffer_id })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub code: u16,
    pub subsystem: u16,
    pub severity: DiagnosticSeverity,
    pub message: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorDirective {
    pub kind: SyntaxKind,
    pub range: Option<Range<usize>>,
    pub directive: Option<PreprocessorDirectiveToken>,
    pub name: Option<PreprocessorDirectiveToken>,
    pub include_file_name: Option<PreprocessorDirectiveToken>,
    pub params: Vec<PreprocessorMacroParam>,
    pub body_tokens: Vec<PreprocessorDirectiveToken>,
    pub expr_tokens: Vec<PreprocessorDirectiveToken>,
    pub disabled_ranges: Vec<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorDirectiveToken {
    pub raw_text: String,
    pub value_text: String,
    pub range: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreprocessorMacroParam {
    pub name: Option<PreprocessorDirectiveToken>,
    pub default_tokens: Option<Vec<PreprocessorDirectiveToken>>,
    pub range: Option<Range<usize>>,
}

impl ParserExpectedSyntax {
    #[inline]
    fn from_raw(raw: ffi::RawExpectedSyntax) -> Self {
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
    fn from_raw(raw: ffi::RawLexedTokenAtOffset) -> Option<Self> {
        raw.has_token.then(|| Self {
            replacement: raw.replacement_start..raw.replacement_end,
            prefix: raw.prefix,
            token_kind: TokenKind::from_id(raw.token_kind),
            directive_kind: raw.has_directive_kind.then(|| SyntaxKind::from_id(raw.directive_kind)),
        })
    }
}

impl PreprocessorDirective {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorDirective) -> Self {
        Self {
            kind: SyntaxKind::from_id(raw.kind),
            range: raw.has_range.then_some(raw.range_start..raw.range_end),
            directive: PreprocessorDirectiveToken::from_raw(raw.directive),
            name: PreprocessorDirectiveToken::from_raw(raw.name),
            include_file_name: PreprocessorDirectiveToken::from_raw(raw.include_file_name),
            params: raw.params.into_iter().map(PreprocessorMacroParam::from_raw).collect(),
            body_tokens: raw
                .body_tokens
                .into_iter()
                .filter_map(PreprocessorDirectiveToken::from_raw)
                .collect(),
            expr_tokens: raw
                .expr_tokens
                .into_iter()
                .filter_map(PreprocessorDirectiveToken::from_raw)
                .collect(),
            disabled_ranges: raw
                .disabled_ranges
                .into_iter()
                .filter_map(|range| range.has_range.then_some(range.range_start..range.range_end))
                .collect(),
        }
    }
}

impl PreprocessorDirectiveToken {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorToken) -> Option<Self> {
        raw.has_token.then(|| Self {
            raw_text: raw.raw_text,
            value_text: raw.value_text,
            range: raw.has_range.then_some(raw.range_start..raw.range_end),
        })
    }
}

impl PreprocessorMacroParam {
    #[inline]
    fn from_raw(raw: ffi::RawPreprocessorMacroParam) -> Self {
        Self {
            name: PreprocessorDirectiveToken::from_raw(raw.name),
            default_tokens: raw.has_default.then(|| {
                raw.default_tokens
                    .into_iter()
                    .filter_map(PreprocessorDirectiveToken::from_raw)
                    .collect()
            }),
            range: raw.has_range.then_some(raw.range_start..raw.range_end),
        }
    }
}

impl SyntaxFacts {
    #[inline]
    pub fn is_possible_statement(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_statement(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_expression(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_expression(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_data_type(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_data_type(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_argument(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_argument(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_param_assignment(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_param_assignment(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_port_connection(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_port_connection(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_ansi_port(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_ansi_port(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_non_ansi_port(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_non_ansi_port(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_function_port(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_function_port(kind.as_u16())
    }

    #[inline]
    pub fn is_possible_parameter(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_possible_parameter(kind.as_u16())
    }

    #[inline]
    pub fn is_gate_type(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_gate_type(kind.as_u16())
    }

    #[inline]
    pub fn is_port_direction(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_port_direction(kind.as_u16())
    }

    #[inline]
    pub fn is_net_type(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_net_type(kind.as_u16())
    }

    #[inline]
    pub fn get_integer_type(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_integer_type(kind.as_u16()))
    }

    #[inline]
    pub fn get_keyword_type(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_keyword_type(kind.as_u16()))
    }

    #[inline]
    pub fn get_procedural_block_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_procedural_block_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_module_declaration_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_module_declaration_kind(kind.as_u16()))
    }

    #[inline]
    pub fn is_possible_member_kind(token_kind: TokenKind, member_kind: SyntaxKind) -> bool {
        ffi::SyntaxToken::is_possible_member_kind(token_kind.as_u16(), member_kind.as_u16())
    }

    #[inline]
    pub fn get_block_item_declaration_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_block_item_declaration_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_library_map_member_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_library_map_member_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_specify_item_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_specify_item_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_config_header_item_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_config_header_item_kind(kind.as_u16()))
    }

    #[inline]
    pub fn get_config_rule_kind(kind: TokenKind) -> SyntaxKind {
        SyntaxKind::from_id(ffi::SyntaxToken::get_config_rule_kind(kind.as_u16()))
    }

    #[inline]
    pub fn keyword_candidates_for_context(
        version: &str,
        context: SyntaxKeywordContext,
    ) -> Vec<String> {
        ffi::SyntaxToken::keyword_candidates_for_context(CxxSV::new(version), context as u8)
    }

    #[inline]
    pub fn is_allowed_in_compilation_unit(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_compilation_unit(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_generate(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_generate(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_module(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_module(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_interface(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_interface(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_program(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_program(kind.as_u16())
    }

    #[inline]
    pub fn is_allowed_in_package(kind: SyntaxKind) -> bool {
        ffi::SyntaxNode::is_allowed_in_package(kind.as_u16())
    }
}

impl SemanticFacts {
    #[inline]
    pub fn is_edge_kind(kind: TokenKind) -> bool {
        ffi::SyntaxToken::is_edge_kind(kind.as_u16())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeUnit {
    Seconds,
    Milliseconds,
    Microseconds,
    Nanoseconds,
    Picoseconds,
    Femtoseconds,
}

impl fmt::Display for TimeUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeUnit::Seconds => write!(f, "s"),
            TimeUnit::Milliseconds => write!(f, "ms"),
            TimeUnit::Microseconds => write!(f, "us"),
            TimeUnit::Nanoseconds => write!(f, "ns"),
            TimeUnit::Picoseconds => write!(f, "ps"),
            TimeUnit::Femtoseconds => write!(f, "fs"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LiteralBase {
    Bin,
    Oct,
    Dec,
    Hex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bit {
    L,
    H,
    X,
    Z,
}

impl fmt::Display for Bit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bit::L => write!(f, "0"),
            Bit::H => write!(f, "1"),
            Bit::X => write!(f, "x"),
            Bit::Z => write!(f, "z"),
        }
    }
}

impl SourceLocation {
    #[cfg(target_pointer_width = "64")]
    const NO_LOCATION: usize = (1usize << 36) - 1;
    #[cfg(target_pointer_width = "32")]
    const NO_LOCATION: usize = usize::MAX;

    #[inline]
    pub fn from_unique_ptr(_ptr: UniquePtr<ffi::SourceLocation>) -> Option<Self> {
        _ptr.is_null().not().then(|| SourceLocation { _ptr })
    }

    #[inline]
    pub fn offset(&self) -> Option<usize> {
        let offset = self._ptr.offset();
        (offset == Self::NO_LOCATION).not().then_some(offset)
    }

    #[inline]
    pub fn buffer_id(&self) -> Option<u32> {
        self.offset().map(|_| self._ptr.buffer_id())
    }
}

impl fmt::Debug for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceLocation")
            .field("buffer_id", &self.buffer_id())
            .field("offset", &self.offset())
            .finish()
    }
}

impl PartialEq for SourceLocation {
    fn eq(&self, other: &Self) -> bool {
        self.buffer_id() == other.buffer_id() && self.offset() == other.offset()
    }
}

impl Eq for SourceLocation {}

impl SourceRange {
    #[inline]
    fn from_unique_ptr(_ptr: UniquePtr<ffi::SourceRange>) -> Option<Self> {
        _ptr.is_null().not().then(|| SourceRange { _ptr })
    }

    #[inline]
    pub fn start(&self) -> usize {
        self._ptr.start()
    }

    #[inline]
    pub fn end(&self) -> usize {
        self._ptr.end()
    }

    #[inline]
    pub fn start_buffer_id(&self) -> u32 {
        self._ptr.start_buffer_id()
    }

    #[inline]
    pub fn end_buffer_id(&self) -> u32 {
        self._ptr.end_buffer_id()
    }

    #[inline]
    pub fn is_single_buffer(&self) -> bool {
        self.start_buffer_id() == self.end_buffer_id()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.start() >= self.end()
    }
}

impl fmt::Debug for SourceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SourceRange")
            .field("start_buffer_id", &self.start_buffer_id())
            .field("start", &self.start())
            .field("end_buffer_id", &self.end_buffer_id())
            .field("end", &self.end())
            .finish()
    }
}

impl PartialEq for SourceRange {
    fn eq(&self, other: &Self) -> bool {
        self.start_buffer_id() == other.start_buffer_id()
            && self.start() == other.start()
            && self.end_buffer_id() == other.end_buffer_id()
            && self.end() == other.end()
    }
}

impl Eq for SourceRange {}

impl SVLogic {
    #[inline]
    pub fn is_unknown(&self) -> bool {
        self._ptr.isUnknown()
    }

    #[inline]
    pub fn char(&self) -> c_char {
        self._ptr.toChar()
    }

    #[inline]
    pub fn bit(&self) -> Bit {
        const X: u8 = 1 << 7;
        const Z: u8 = 1 << 6;
        match self._ptr.value() {
            0 => Bit::L,
            1 => Bit::H,
            X => Bit::X,
            Z => Bit::Z,
            _ => unreachable!(),
        }
    }
}

impl SVInt {
    #[inline]
    pub fn is_signed(&self) -> bool {
        self._ptr.isSigned()
    }

    #[inline]
    pub fn has_unknown(&self) -> bool {
        self._ptr.hasUnknown()
    }

    #[inline]
    pub fn get_bit_width(&self) -> usize {
        self._ptr.getBitWidth() as usize
    }

    #[inline]
    pub fn is_single_word(&self) -> bool {
        const CHAR_BIT: usize = core::ffi::c_char::BITS as usize;
        const BITS_PER_WORD: usize = core::mem::size_of::<u64>() * CHAR_BIT;
        self.get_bit_width() <= BITS_PER_WORD && !self.has_unknown()
    }

    #[inline]
    pub fn get_single_word(&self) -> Option<u64> {
        self.is_single_word().then(|| unsafe { *self._ptr.getRawPtr() })
    }

    #[inline]
    pub fn logic_eq(&self, other: &SVInt) -> SVLogic {
        let logic = self._ptr.eq(&other._ptr);
        SVLogic { _ptr: logic }
    }

    #[inline]
    pub fn serialize(&self, base: usize) -> String {
        self._ptr.toString(base)
    }
}

unsafe impl Send for SVInt {}

unsafe impl Sync for SVInt {}

impl fmt::Debug for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SVInt").field("to_string", &self.to_string()).finish()
    }
}

impl Clone for SVInt {
    fn clone(&self) -> Self {
        SVInt { _ptr: self._ptr.clone() }
    }
}

impl PartialEq for SVInt {
    fn eq(&self, other: &Self) -> bool {
        let logic = self.logic_eq(other);
        logic.bit() == Bit::H
    }
}

impl Eq for SVInt {}

impl hash::Hash for SVInt {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self._ptr.getRawPtr().hash(state)
    }
}

impl Display for SVInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self._ptr.toString(10))
    }
}

impl fmt::Debug for SyntaxTrivia<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxTrivia")
            .field("kind", &self.kind())
            .field("raw_text", &self.get_raw_text())
            .finish()
    }
}

pub trait ChildrenIter<It> = DoubleEndedIterator<Item = It> + ExactSizeIterator + Clone;

impl<'a> SyntaxTrivia<'a> {
    #[inline]
    fn from_raw_ptr(_ptr: *const ffi::SyntaxTrivia) -> Option<Self> {
        assert!(_ptr.is_null().not());
        Some(SyntaxTrivia { _ptr: unsafe { Pin::new_unchecked(&*_ptr) } })
    }

    #[inline]
    pub fn get_raw_text(&self) -> CxxSV<'_> {
        self._ptr.getRawText()
    }

    #[inline]
    pub fn kind(&self) -> TriviaKind {
        TriviaKind::from_id(self._ptr.kind())
    }

    #[inline]
    fn explicit_location(&self) -> Option<SourceLocation> {
        SourceLocation::from_unique_ptr(self._ptr.getExplicitLocation())
    }

    #[inline]
    pub fn syntax(&self) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_raw_ptr(self._ptr.syntax())
    }
}

impl<'a> SyntaxToken<'a> {
    #[inline]
    pub fn keyword_table_for_version(version: &str) -> Vec<String> {
        ffi::SyntaxToken::keyword_table_for_version(CxxSV::new(version))
    }

    #[inline]
    pub fn keyword_kind_for_version(version: &str, text: &str) -> TokenKind {
        TokenKind::from_id(ffi::SyntaxToken::keyword_kind_for_version(
            CxxSV::new(version),
            CxxSV::new(text),
        ))
    }

    #[inline]
    pub fn verilog_2005_keywords() -> Vec<String> {
        ffi::SyntaxToken::verilog_2005_keywords()
    }

    #[inline]
    pub fn directive_text(kind: SyntaxKind) -> String {
        ffi::SyntaxToken::directive_text(kind.as_u16())
    }

    #[inline]
    fn from_raw_ptr(_ptr: *const ffi::SyntaxToken) -> Option<Self> {
        _ptr.is_null().not().then(|| SyntaxToken { _ptr: unsafe { Pin::new_unchecked(&*_ptr) } })
    }

    #[inline]
    pub fn is_missing(&self) -> bool {
        self._ptr.isMissing()
    }

    #[inline]
    pub fn range(&self) -> Option<SourceRange> {
        SourceRange::from_unique_ptr(self._ptr.range())
    }

    #[inline]
    pub fn value_text(&self) -> CxxSV<'_> {
        self._ptr.valueText()
    }

    #[inline]
    pub fn raw_text(&self) -> CxxSV<'_> {
        self._ptr.rawText()
    }

    #[inline]
    pub fn kind(&self) -> TokenKind {
        TokenKind::from_id(self._ptr.kind())
    }

    #[inline]
    pub fn int(&self) -> Option<SVInt> {
        matches!(self.kind(), TokenKind::INTEGER_LITERAL)
            .then(|| SVInt { _ptr: self._ptr.intValue() })
    }

    #[inline]
    pub fn bits(&self) -> Option<SVLogic> {
        matches!(self.kind(), TokenKind::UNBASED_UNSIZED_LITERAL)
            .then(|| SVLogic { _ptr: self._ptr.bitValue() })
    }

    #[inline]
    pub fn real(&self) -> Option<f64> {
        matches!(self.kind(), TokenKind::REAL_LITERAL | TokenKind::TIME_LITERAL)
            .then(|| self._ptr.realValue())
    }

    #[inline]
    pub fn base(&self) -> Option<LiteralBase> {
        matches!(self.kind(), TokenKind::INTEGER_BASE)
            .then(|| unsafe { std::mem::transmute::<u8, LiteralBase>(self._ptr.base()) })
    }

    #[inline]
    pub fn time_unit(&self) -> Option<TimeUnit> {
        matches!(self.kind(), TokenKind::TIME_LITERAL)
            .then(|| unsafe { std::mem::transmute::<u8, TimeUnit>(self._ptr.unit()) })
    }

    #[inline]
    pub fn trivia_count(&self) -> usize {
        self._ptr.trivia_count()
    }

    #[inline]
    pub fn trivia_at(&self, idx: usize) -> Option<SyntaxTrivia<'a>> {
        SyntaxTrivia::from_raw_ptr(self._ptr.trivia(idx))
    }

    #[inline]
    pub fn trivias(&self) -> impl ChildrenIter<SyntaxTrivia<'a>> + use<'a> {
        SyntaxTriviaIter { tok: *self, idx: 0, total: self.trivia_count() }
    }

    #[inline]
    pub fn trivias_with_loc(
        &self,
    ) -> impl ChildrenIter<(SyntaxTriviaLoc, SyntaxTrivia<'a>)> + use<'a> {
        let Some(range) = self.range() else {
            return Either::Left(iter::empty());
        };
        let mut cursor_buffer_id = range.start_buffer_id();
        let mut cursor_offset = range.start();
        let mut locs = Vec::with_capacity(self.trivia_count());

        for trivia in self.trivias().rev() {
            let len = trivia.get_raw_text().as_bytes().len();

            let loc = if let Some(location) = trivia.explicit_location() {
                let start = location.offset().unwrap();
                SyntaxTriviaLoc {
                    buffer_id: location.buffer_id().unwrap(),
                    start,
                    end: start + len,
                }
            } else {
                let end = cursor_offset;
                let start = end - len;
                SyntaxTriviaLoc { buffer_id: cursor_buffer_id, start, end }
            };

            cursor_buffer_id = loc.buffer_id;
            cursor_offset = loc.start;
            locs.push((loc, trivia));
        }

        Either::Right(locs.into_iter().rev())
    }
}

impl fmt::Debug for SyntaxToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxToken")
            .field("kind", &self.kind())
            .field("range", &self.range())
            .field("value_text", &self.value_text())
            .finish()
    }
}

impl PartialEq for SyntaxToken<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind() && self.range() == other.range()
    }
}

impl Eq for SyntaxToken<'_> {}

#[derive(Debug, Clone)]
pub struct SyntaxTriviaIter<'a> {
    tok: SyntaxToken<'a>,
    idx: usize,
    total: usize,
}

impl<'a> Iterator for SyntaxTriviaIter<'a> {
    type Item = SyntaxTrivia<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx < self.total {
            let trivia = self.tok.trivia_at(self.idx).unwrap();
            self.idx += 1;
            Some(trivia)
        } else {
            None
        }
    }
}

impl<'a> DoubleEndedIterator for SyntaxTriviaIter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.idx < self.total {
            self.total -= 1;
            let trivia = self.tok.trivia_at(self.total).unwrap();
            Some(trivia)
        } else {
            None
        }
    }
}

impl<'a> ExactSizeIterator for SyntaxTriviaIter<'a> {
    fn len(&self) -> usize {
        self.total - self.idx
    }
}

impl<'a> SyntaxNode<'a> {
    #[inline]
    fn from_raw_ptr(_ptr: *const ffi::SyntaxNode) -> Option<Self> {
        _ptr.is_null().not().then(|| SyntaxNode { _ptr: unsafe { Pin::new_unchecked(&*_ptr) } })
    }

    #[inline]
    pub fn walk(&self) -> SyntaxCursor<'a> {
        SyntaxCursor::new(*self)
    }

    #[inline]
    pub fn range(&self) -> Option<SourceRange> {
        SourceRange::from_unique_ptr(self._ptr.range())
    }

    #[inline]
    pub fn range_with_context(&self, context: SyntaxNode<'a>) -> Option<SourceRange> {
        let node = self._ptr.as_ref().get_ref();
        let context = context._ptr.as_ref().get_ref();
        SourceRange::from_unique_ptr(ffi::SyntaxNode::rangeWithContext(node, context))
    }

    #[inline]
    pub fn child_node(&self, idx: usize) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_raw_ptr(self._ptr.childNode(idx))
    }

    // not-null
    #[inline]
    pub fn child_token(&self, idx: usize) -> Option<SyntaxToken<'a>> {
        SyntaxToken::from_raw_ptr(self._ptr.childToken(idx))
            .filter(|tok| tok.kind() != TokenKind::UNKNOWN)
    }

    #[inline]
    pub fn child_count(&self) -> usize {
        self._ptr.getChildCount()
    }

    #[inline]
    pub fn kind(&self) -> SyntaxKind {
        SyntaxKind::from_id(self._ptr.kind())
    }

    #[inline]
    pub fn parent(&self) -> Option<SyntaxNode<'a>> {
        SyntaxNode::from_raw_ptr(self._ptr.parent())
    }

    #[inline]
    pub fn child(&self, idx: usize) -> Option<SyntaxElement<'a>> {
        // TODO: we have to visit twice to get the child, this is not efficient
        if idx >= self.child_count() {
            None
        } else if let Some(node) = self.child_node(idx) {
            Some(SyntaxElement::Node(node))
        } else {
            self.child_token(idx)
                .map(|tok| SyntaxElement::Token(SyntaxTokenWithParent { parent: *self, tok }))
        }
    }

    #[inline]
    pub fn children_with_idx(&self) -> SyntaxIdxChildren<'a> {
        SyntaxIdxChildren::new(*self)
    }

    #[inline]
    pub fn children(&self) -> SyntaxChildren<'a> {
        SyntaxChildren::new(*self)
    }

    #[inline]
    pub fn elem_preorder(&self) -> SyntaxElemPreorder<'a> {
        SyntaxElemPreorder::new(*self)
    }

    #[inline]
    pub fn node_preorder(&self) -> SyntaxNodePreorder<'a> {
        SyntaxNodePreorder::new(*self)
    }

    #[inline]
    pub fn first_token(&self) -> Option<SyntaxTokenWithParent<'a>> {
        let mut cursor = self.walk();

        while cursor.to_tok_with_parent().is_none() {
            if cursor.goto_first_child() {
                continue;
            }

            while !cursor.goto_next_sibling() {
                if !cursor.goto_parent() {
                    unreachable!("Tree has no tokens");
                }
            }
        }

        cursor.to_tok_with_parent()
    }
}

impl fmt::Debug for SyntaxNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxNode")
            .field("kind", &self.kind())
            .field("range", &self.range())
            .field("child_count", &self.child_count())
            .finish()
    }
}

impl PartialEq for SyntaxNode<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // Just compare pointer
        std::ptr::eq(Pin::as_ref(&self._ptr).get_ref(), Pin::as_ref(&other._ptr).get_ref())
    }
}

impl Eq for SyntaxNode<'_> {}

impl hash::Hash for SyntaxNode<'_> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        let ptr = Pin::as_ref(&self._ptr).get_ref() as *const ffi::SyntaxNode;
        ptr.hash(state)
    }
}

impl SyntaxTree {
    #[inline]
    pub fn from_text(text: &str, name: &str, path: &str) -> SyntaxTree {
        SyntaxTree {
            _ptr: ffi::SyntaxTree::fromText(CxxSV::new(text), CxxSV::new(name), CxxSV::new(path)),
        }
    }

    #[inline]
    pub fn from_text_with_options(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTree {
        SyntaxTree {
            _ptr: ffi::SyntaxTree::fromTextWithOptions(
                CxxSV::new(text),
                CxxSV::new(name),
                CxxSV::new(path),
                options.predefines.clone(),
                options.include_paths.clone(),
                options
                    .include_buffers
                    .iter()
                    .map(|buffer| ffi::RawSourceBuffer {
                        path: buffer.path.clone(),
                        text: buffer.text.clone(),
                    })
                    .collect(),
                options.expand_includes,
            ),
        }
    }

    #[inline]
    pub fn from_library_map_text(text: &str, name: &str, path: &str) -> SyntaxTree {
        SyntaxTree {
            _ptr: ffi::SyntaxTree::fromLibraryMapText(
                CxxSV::new(text),
                CxxSV::new(name),
                CxxSV::new(path),
            ),
        }
    }

    #[inline]
    pub fn root(&self) -> Option<SyntaxNode<'_>> {
        SyntaxNode::from_raw_ptr(self._ptr.root())
    }

    pub fn diagnostics(&self) -> Vec<SyntaxDiagnostic> {
        self._ptr.diagnostics().into_iter().map(SyntaxDiagnostic::from_raw).collect()
    }

    pub fn diagnostics_with_options(&self, warning_options: &[String]) -> Vec<SyntaxDiagnostic> {
        self._ptr
            .diagnostics_with_options(warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
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
        ffi::SyntaxTree::expectedSyntaxAtOffset(
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
            offset,
            options.predefines.clone(),
            options.include_paths.clone(),
            options
                .include_buffers
                .iter()
                .map(|buffer| ffi::RawSourceBuffer {
                    path: buffer.path.clone(),
                    text: buffer.text.clone(),
                })
                .collect(),
            options.expand_includes,
        )
        .into_iter()
        .map(ParserExpectedSyntax::from_raw)
        .collect()
    }

    pub fn library_map_expected_syntax_at_offset(
        text: &str,
        name: &str,
        path: &str,
        offset: usize,
    ) -> Vec<ParserExpectedSyntax> {
        ffi::SyntaxTree::libraryMapExpectedSyntaxAtOffset(
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
            offset,
        )
        .into_iter()
        .map(ParserExpectedSyntax::from_raw)
        .collect()
    }

    pub fn directive_at_offset(
        text: &str,
        name: &str,
        path: &str,
        offset: usize,
    ) -> Option<LexedTokenAtOffset> {
        LexedTokenAtOffset::from_raw(ffi::SyntaxTree::directiveAtOffset(
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
            offset,
        ))
    }

    pub fn token_word_at_offset(
        text: &str,
        name: &str,
        path: &str,
        offset: usize,
    ) -> Option<LexedTokenAtOffset> {
        LexedTokenAtOffset::from_raw(ffi::SyntaxTree::tokenWordAtOffset(
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
            offset,
        ))
    }

    pub fn preprocessor_directives(
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> Vec<PreprocessorDirective> {
        ffi::SyntaxTree::preprocessorDirectives(
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
            options.predefines.clone(),
        )
        .into_iter()
        .map(PreprocessorDirective::from_raw)
        .collect()
    }

    pub fn buffer_id(&self) -> u32 {
        self._ptr.buffer_id()
    }
}

unsafe impl Send for SyntaxTree {}

unsafe impl Sync for SyntaxTree {}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("<SyntaxTree>").finish()
    }
}

impl PartialEq for SyntaxTree {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let a = self._ptr.as_ref().unwrap();
        let b = other._ptr.as_ref().unwrap();
        std::ptr::eq(std::ptr::from_ref(a), std::ptr::from_ref(b))
    }
}

impl Eq for SyntaxTree {}

#[derive(Debug, Clone, Copy)]
pub struct SyntaxTokenWithParent<'a> {
    pub parent: SyntaxNode<'a>,
    pub tok: SyntaxToken<'a>,
}

impl SyntaxTokenWithParent<'_> {
    #[inline]
    pub fn range(&self) -> Option<SourceRange> {
        let token = self.tok._ptr.as_ref().get_ref();
        let context = self.parent._ptr.as_ref().get_ref();
        SourceRange::from_unique_ptr(ffi::SyntaxToken::rangeWithContext(token, context))
    }
}

impl<'a> std::ops::Deref for SyntaxTokenWithParent<'a> {
    type Target = SyntaxToken<'a>;

    fn deref(&self) -> &Self::Target {
        &self.tok
    }
}

impl PartialEq for SyntaxTokenWithParent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent && self.tok == other.tok
    }
}

impl Eq for SyntaxTokenWithParent<'_> {}

#[derive(Debug, Clone, Copy)]
pub enum SyntaxElement<'a> {
    Node(SyntaxNode<'a>),
    Token(SyntaxTokenWithParent<'a>),
}

impl<'a> SyntaxElement<'a> {
    pub fn from_node(node: SyntaxNode) -> SyntaxElement {
        SyntaxElement::Node(node)
    }

    pub fn from_token<'b>(tok_with_parent: SyntaxTokenWithParent<'b>) -> SyntaxElement<'b> {
        SyntaxElement::Token(tok_with_parent)
    }

    pub fn as_node(&self) -> Option<SyntaxNode<'a>> {
        match self {
            SyntaxElement::Node(node) => Some(*node),
            SyntaxElement::Token(_) => None,
        }
    }

    pub fn as_tok_with_parent(&self) -> Option<SyntaxTokenWithParent<'a>> {
        match self {
            SyntaxElement::Token(tok_with_parent) => Some(*tok_with_parent),
            SyntaxElement::Node(_) => None,
        }
    }

    pub fn as_token(&self) -> Option<SyntaxToken<'a>> {
        match self {
            SyntaxElement::Token(tok_with_parent) => Some(tok_with_parent.tok),
            SyntaxElement::Node(_) => None,
        }
    }

    pub fn child_count(&self) -> usize {
        match self {
            SyntaxElement::Node(node) => node.child_count(),
            SyntaxElement::Token { .. } => 0,
        }
    }

    pub fn child(&self, idx: usize) -> Option<SyntaxElement<'a>> {
        match self {
            SyntaxElement::Node(node) => node.child(idx),
            SyntaxElement::Token { .. } => None,
        }
    }

    pub fn range(&self) -> Option<SourceRange> {
        match self {
            SyntaxElement::Node(node) => node.range(),
            SyntaxElement::Token(tok) => tok.range(),
        }
    }

    pub fn parent(&self) -> Option<SyntaxNode<'a>> {
        match self {
            SyntaxElement::Node(node) => node.parent(),
            SyntaxElement::Token(tok) => Some(tok.parent),
        }
    }

    pub fn kind(&self) -> SyntaxElementKind {
        match self {
            SyntaxElement::Node(node) => SyntaxElementKind::Node(node.kind()),
            SyntaxElement::Token(tok) => SyntaxElementKind::Token(tok.kind()),
        }
    }

    pub fn children_with_idx(
        &self,
    ) -> Either<SyntaxIdxChildren<'a>, iter::Empty<(usize, SyntaxElement<'a>)>> {
        match self {
            SyntaxElement::Node(node) => Either::Left(node.children_with_idx()),
            SyntaxElement::Token(_) => Either::Right(iter::empty()),
        }
    }

    pub fn children(&self) -> Either<SyntaxChildren<'a>, iter::Empty<SyntaxElement<'a>>> {
        match self {
            SyntaxElement::Node(node) => Either::Left(node.children()),
            SyntaxElement::Token(_) => Either::Right(iter::empty()),
        }
    }
}

impl<'a> From<SyntaxNode<'a>> for SyntaxElement<'a> {
    fn from(node: SyntaxNode<'a>) -> SyntaxElement<'a> {
        SyntaxElement::Node(node)
    }
}

impl<'a> From<SyntaxTokenWithParent<'a>> for SyntaxElement<'a> {
    fn from(tok_with_parent: SyntaxTokenWithParent<'a>) -> SyntaxElement<'a> {
        SyntaxElement::Token(tok_with_parent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxElementKind {
    Node(SyntaxKind),
    Token(TokenKind),
}

impl SyntaxElementKind {
    pub fn is_list(&self) -> bool {
        match self {
            SyntaxElementKind::Node(kind) => kind.is_list(),
            SyntaxElementKind::Token(_) => false,
        }
    }
}

impl From<SyntaxKind> for SyntaxElementKind {
    fn from(kind: SyntaxKind) -> SyntaxElementKind {
        SyntaxElementKind::Node(kind)
    }
}

impl From<TokenKind> for SyntaxElementKind {
    fn from(kind: TokenKind) -> SyntaxElementKind {
        SyntaxElementKind::Token(kind)
    }
}

pub struct Compilation {
    _ptr: UniquePtr<ffi::Compilation>,
}

impl Default for Compilation {
    fn default() -> Self {
        Self::new()
    }
}

impl Compilation {
    pub fn new() -> Self {
        Compilation { _ptr: ffi::Compilation::new() }
    }

    pub fn new_with_top_modules(top_modules: &[String]) -> Self {
        Compilation { _ptr: ffi::Compilation::new_with_top_modules(top_modules.to_vec()) }
    }

    pub fn system_function_names() -> Vec<String> {
        ffi::Compilation::system_function_names()
    }

    pub fn system_task_names() -> Vec<String> {
        ffi::Compilation::system_task_names()
    }

    pub fn add_syntax_tree(&mut self, tree: SyntaxTree) {
        ffi::Compilation::add_syntax_tree(self._ptr.as_mut().unwrap(), tree._ptr);
    }

    pub fn add_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
        options: &SyntaxTreeOptions,
    ) -> SyntaxTreeBufferIds {
        SyntaxTreeBufferIds::from_raw(ffi::Compilation::add_syntax_tree_from_text(
            self._ptr.as_mut().unwrap(),
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
            options.predefines.clone(),
            options.include_paths.clone(),
            options
                .include_buffers
                .iter()
                .map(|buffer| ffi::RawSourceBuffer {
                    path: buffer.path.clone(),
                    text: buffer.text.clone(),
                })
                .collect(),
            options.expand_includes,
        ))
    }

    pub fn add_library_map_syntax_tree_from_text(
        &mut self,
        text: &str,
        name: &str,
        path: &str,
    ) -> SyntaxTreeBufferIds {
        SyntaxTreeBufferIds::from_raw(ffi::Compilation::add_library_map_syntax_tree_from_text(
            self._ptr.as_mut().unwrap(),
            CxxSV::new(text),
            CxxSV::new(name),
            CxxSV::new(path),
        ))
    }

    pub fn semantic_diagnostics(&self) -> Vec<SyntaxDiagnostic> {
        self._ptr.semantic_diagnostics().into_iter().map(SyntaxDiagnostic::from_raw).collect()
    }

    pub fn parse_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        self._ptr
            .parse_diagnostics_with_options(warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
    }

    pub fn semantic_diagnostics_with_options(
        &self,
        warning_options: &[String],
    ) -> Vec<SyntaxDiagnostic> {
        self._ptr
            .semantic_diagnostics_with_options(warning_options.to_vec())
            .into_iter()
            .map(SyntaxDiagnostic::from_raw)
            .collect()
    }
}

#[cfg(test)]
mod tests;
