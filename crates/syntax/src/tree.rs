use std::{fmt, hash, iter, ops::Range, ptr};

use either::Either;
use smol_str::SmolStr;
use utils::line_index::{TextRange, TextSize};

use crate::{Bit, LiteralBase, SVInt, SVLogic, SyntaxKind, TimeUnit, TokenKind, TriviaKind};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceRange {
    buffer_id: u32,
    start: usize,
    end: usize,
}

impl SourceRange {
    pub fn new(buffer_id: u32, start: usize, end: usize) -> Self {
        Self { buffer_id, start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn start_buffer_id(&self) -> u32 {
        self.buffer_id
    }

    pub fn end_buffer_id(&self) -> u32 {
        self.buffer_id
    }

    pub fn is_single_buffer(&self) -> bool {
        true
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    pub fn text_range(self) -> Option<TextRange> {
        let start = u32::try_from(self.start).ok()?;
        let end = u32::try_from(self.end).ok()?;
        Some(TextRange::new(TextSize::new(start), TextSize::new(end)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxTriviaLoc {
    pub buffer_id: u32,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedTrivia {
    pub kind: TriviaKind,
    pub raw_text: SmolStr,
    pub range: Option<TextRange>,
}

#[derive(Debug, Clone)]
struct NodeData {
    kind: SyntaxKind,
    range: Option<TextRange>,
    parent: Option<usize>,
    children: Vec<Option<ElementId>>,
}

#[derive(Debug, Clone)]
struct TokenData {
    kind: TokenKind,
    raw_text: SmolStr,
    value_text: SmolStr,
    range: Option<TextRange>,
    parent: usize,
    trivia: Vec<OwnedTrivia>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ElementId {
    Node(usize),
    Token(usize),
}

#[derive(Debug, Clone)]
struct TreeData {
    source: String,
    buffer_id: u32,
    root: usize,
    nodes: Vec<NodeData>,
    tokens: Vec<TokenData>,
}

#[derive(Clone)]
pub struct SyntaxTree {
    data: std::sync::Arc<TreeData>,
}

impl SyntaxTree {
    pub fn empty() -> Self {
        let mut builder = SyntaxTreeBuilder::new(String::new(), 0);
        builder.start_root(
            SyntaxKind::COMPILATION_UNIT,
            0,
            Some(TextRange::empty(TextSize::new(0))),
        );
        builder.finish_node();
        builder.finish()
    }

    pub fn from_builder(builder: SyntaxTreeBuilder) -> Self {
        builder.finish()
    }

    pub fn root(&self) -> Option<SyntaxNode<'_>> {
        Some(SyntaxNode { tree: &self.data, id: self.data.root })
    }

    pub fn text(&self) -> &str {
        &self.data.source
    }

    pub fn buffer_id(&self) -> u32 {
        self.data.buffer_id
    }
}

impl fmt::Debug for SyntaxTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxTree").field("root", &self.root()).finish()
    }
}

impl PartialEq for SyntaxTree {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.data, &other.data)
    }
}

impl Eq for SyntaxTree {}

pub struct SyntaxTreeBuilder {
    source: String,
    buffer_id: u32,
    nodes: Vec<NodeData>,
    tokens: Vec<TokenData>,
    stack: Vec<usize>,
    root: Option<usize>,
}

impl SyntaxTreeBuilder {
    pub fn new(source: String, buffer_id: u32) -> Self {
        Self {
            source,
            buffer_id,
            nodes: Vec::new(),
            tokens: Vec::new(),
            stack: Vec::new(),
            root: None,
        }
    }

    pub fn start_root(&mut self, kind: SyntaxKind, child_count: usize, range: Option<TextRange>) {
        assert!(self.root.is_none(), "syntax tree root should only be started once");
        let id = self.push_node(kind, child_count, range, None);
        self.root = Some(id);
        self.stack.push(id);
    }

    pub fn start_child_node(
        &mut self,
        slot: usize,
        kind: SyntaxKind,
        child_count: usize,
        range: Option<TextRange>,
    ) {
        let parent = *self.stack.last().expect("child node needs a parent");
        let id = self.push_node(kind, child_count, range, Some(parent));
        self.set_child(parent, slot, ElementId::Node(id));
        self.stack.push(id);
    }

    pub fn finish_node(&mut self) {
        self.stack.pop().expect("finish_node without matching start_node");
    }

    pub fn token(
        &mut self,
        slot: usize,
        kind: TokenKind,
        raw_text: impl Into<SmolStr>,
        value_text: impl Into<SmolStr>,
        range: Option<TextRange>,
        trivia: Vec<OwnedTrivia>,
    ) {
        let parent = *self.stack.last().expect("token needs a parent node");
        let id = self.tokens.len();
        self.tokens.push(TokenData {
            kind,
            raw_text: raw_text.into(),
            value_text: value_text.into(),
            range,
            parent,
            trivia,
        });
        self.set_child(parent, slot, ElementId::Token(id));
    }

    fn push_node(
        &mut self,
        kind: SyntaxKind,
        child_count: usize,
        range: Option<TextRange>,
        parent: Option<usize>,
    ) -> usize {
        let id = self.nodes.len();
        self.nodes.push(NodeData { kind, range, parent, children: vec![None; child_count] });
        id
    }

    fn set_child(&mut self, parent: usize, slot: usize, child: ElementId) {
        let slots = &mut self.nodes[parent].children;
        if slot >= slots.len() {
            slots.resize(slot + 1, None);
        }
        slots[slot] = Some(child);
    }

    fn finish(self) -> SyntaxTree {
        assert!(self.stack.is_empty(), "syntax tree builder finished with open nodes");
        let root = self.root.expect("syntax tree builder finished without root");
        SyntaxTree {
            data: std::sync::Arc::new(TreeData {
                source: self.source,
                buffer_id: self.buffer_id,
                root,
                nodes: self.nodes,
                tokens: self.tokens,
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub struct SyntaxNode<'a> {
    tree: &'a TreeData,
    id: usize,
}

impl<'a> SyntaxNode<'a> {
    pub fn walk(&self) -> SyntaxCursor<'a> {
        SyntaxCursor::new(*self)
    }

    pub fn range(&self) -> Option<SourceRange> {
        self.text_range().map(|range| {
            SourceRange::new(self.tree.buffer_id, range.start().into(), range.end().into())
        })
    }

    pub fn range_with_context(&self, _context: SyntaxNode<'a>) -> Option<SourceRange> {
        self.range()
    }

    pub fn text_range(&self) -> Option<TextRange> {
        self.data().range
    }

    pub fn child_node(&self, idx: usize) -> Option<SyntaxNode<'a>> {
        match self.data().children.get(idx).copied().flatten()? {
            ElementId::Node(id) => Some(SyntaxNode { tree: self.tree, id }),
            ElementId::Token(_) => None,
        }
    }

    pub fn child_token(&self, idx: usize) -> Option<SyntaxToken<'a>> {
        match self.data().children.get(idx).copied().flatten()? {
            ElementId::Node(_) => None,
            ElementId::Token(id) => {
                let token = SyntaxToken { tree: self.tree, id };
                (token.kind() != TokenKind::UNKNOWN).then_some(token)
            }
        }
    }

    pub fn child_count(&self) -> usize {
        self.data().children.len()
    }

    pub fn kind(&self) -> SyntaxKind {
        self.data().kind
    }

    pub fn parent(&self) -> Option<SyntaxNode<'a>> {
        self.data().parent.map(|id| SyntaxNode { tree: self.tree, id })
    }

    pub fn child(&self, idx: usize) -> Option<SyntaxElement<'a>> {
        match self.data().children.get(idx).copied().flatten()? {
            ElementId::Node(id) => Some(SyntaxElement::Node(SyntaxNode { tree: self.tree, id })),
            ElementId::Token(id) => Some(SyntaxElement::Token(SyntaxTokenWithParent {
                parent: *self,
                tok: SyntaxToken { tree: self.tree, id },
            })),
        }
    }

    pub fn children_with_idx(&self) -> SyntaxIdxChildren<'a> {
        SyntaxIdxChildren::new(*self)
    }

    pub fn children(&self) -> SyntaxChildren<'a> {
        SyntaxChildren::new(*self)
    }

    pub fn elem_preorder(&self) -> SyntaxElemPreorder<'a> {
        SyntaxElemPreorder::new(*self)
    }

    pub fn node_preorder(&self) -> SyntaxNodePreorder<'a> {
        SyntaxNodePreorder::new(*self)
    }

    pub fn first_token(&self) -> Option<SyntaxTokenWithParent<'a>> {
        self.elem_preorder().find_map(|event| match event {
            WalkEvent::Enter(SyntaxElement::Token(token)) => Some(token),
            _ => None,
        })
    }

    fn data(&self) -> &NodeData {
        &self.tree.nodes[self.id]
    }
}

impl fmt::Debug for SyntaxNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxNode")
            .field("kind", &self.kind())
            .field("range", &self.text_range())
            .field("child_count", &self.child_count())
            .finish()
    }
}

impl PartialEq for SyntaxNode<'_> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.tree, other.tree) && self.id == other.id
    }
}

impl Eq for SyntaxNode<'_> {}

impl hash::Hash for SyntaxNode<'_> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        ptr::hash(self.tree, state);
        self.id.hash(state);
    }
}

#[derive(Clone, Copy)]
pub struct SyntaxToken<'a> {
    tree: &'a TreeData,
    id: usize,
}

impl<'a> SyntaxToken<'a> {
    pub fn keyword_table_for_version(_version: &str) -> Vec<String> {
        keyword_texts().into_iter().map(str::to_owned).collect()
    }

    pub fn keyword_kind_for_version(_version: &str, text: &str) -> TokenKind {
        keyword_kind(text).unwrap_or(TokenKind::IDENTIFIER)
    }

    pub fn verilog_2005_keywords() -> Vec<String> {
        keyword_texts().into_iter().map(str::to_owned).collect()
    }

    pub fn directive_text(kind: SyntaxKind) -> String {
        match kind {
            SyntaxKind::DEFINE_DIRECTIVE => "`define",
            SyntaxKind::UNDEF_DIRECTIVE => "`undef",
            SyntaxKind::INCLUDE_DIRECTIVE => "`include",
            SyntaxKind::IF_DEF_DIRECTIVE => "`ifdef",
            SyntaxKind::IF_N_DEF_DIRECTIVE => "`ifndef",
            SyntaxKind::ELS_IF_DIRECTIVE => "`elsif",
            SyntaxKind::ELSE_DIRECTIVE => "`else",
            SyntaxKind::END_IF_DIRECTIVE => "`endif",
            _ => "",
        }
        .to_owned()
    }

    pub fn is_missing(&self) -> bool {
        self.kind() == TokenKind::UNKNOWN && self.raw_text().is_empty()
    }

    pub fn range(&self) -> Option<SourceRange> {
        self.text_range().map(|range| {
            SourceRange::new(self.tree.buffer_id, range.start().into(), range.end().into())
        })
    }

    pub fn text_range(&self) -> Option<TextRange> {
        self.data().range
    }

    pub fn value_text(&self) -> &str {
        &self.data().value_text
    }

    pub fn raw_text(&self) -> &str {
        &self.data().raw_text
    }

    pub fn kind(&self) -> TokenKind {
        self.data().kind
    }

    pub fn int(&self) -> Option<SVInt> {
        if self.kind() != TokenKind::INTEGER_LITERAL {
            return None;
        }
        SVInt::from_literal(self.raw_text())
    }

    pub fn bits(&self) -> Option<SVLogic> {
        if self.kind() != TokenKind::UNBASED_UNSIZED_LITERAL {
            return None;
        }
        let bit = self.raw_text().chars().rev().find_map(|ch| match ch {
            '0' => Some(Bit::L),
            '1' => Some(Bit::H),
            'x' | 'X' => Some(Bit::X),
            'z' | 'Z' | '?' => Some(Bit::Z),
            _ => None,
        })?;
        Some(SVLogic::new(bit))
    }

    pub fn real(&self) -> Option<f64> {
        matches!(self.kind(), TokenKind::REAL_LITERAL | TokenKind::TIME_LITERAL)
            .then(|| numeric_prefix(self.raw_text()).parse::<f64>().ok())?
    }

    pub fn base(&self) -> Option<LiteralBase> {
        if self.kind() != TokenKind::INTEGER_BASE {
            return None;
        }
        self.raw_text().chars().find_map(|ch| match ch {
            'b' | 'B' => Some(LiteralBase::Bin),
            'o' | 'O' => Some(LiteralBase::Oct),
            'd' | 'D' => Some(LiteralBase::Dec),
            'h' | 'H' => Some(LiteralBase::Hex),
            _ => None,
        })
    }

    pub fn time_unit(&self) -> Option<TimeUnit> {
        if self.kind() != TokenKind::TIME_LITERAL {
            return None;
        }
        let raw = self.raw_text().trim_ascii().to_ascii_lowercase();
        if raw.ends_with("fs") {
            Some(TimeUnit::Femtoseconds)
        } else if raw.ends_with("ps") {
            Some(TimeUnit::Picoseconds)
        } else if raw.ends_with("ns") {
            Some(TimeUnit::Nanoseconds)
        } else if raw.ends_with("us") {
            Some(TimeUnit::Microseconds)
        } else if raw.ends_with("ms") {
            Some(TimeUnit::Milliseconds)
        } else if raw.ends_with('s') {
            Some(TimeUnit::Seconds)
        } else {
            None
        }
    }

    pub fn trivia_count(&self) -> usize {
        self.data().trivia.len()
    }

    pub fn trivia_at(&self, idx: usize) -> Option<SyntaxTrivia<'a>> {
        (idx < self.trivia_count()).then_some(SyntaxTrivia { tree: self.tree, token: self.id, idx })
    }

    pub fn trivias(&self) -> SyntaxTriviaIter<'a> {
        SyntaxTriviaIter { token: *self, start: 0, end: self.trivia_count() }
    }

    pub fn trivias_with_loc(&self) -> SyntaxTriviaWithLocIter<'a> {
        SyntaxTriviaWithLocIter { token: *self, start: 0, end: self.trivia_count() }
    }

    fn data(&self) -> &TokenData {
        &self.tree.tokens[self.id]
    }
}

impl fmt::Debug for SyntaxToken<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyntaxToken")
            .field("kind", &self.kind())
            .field("range", &self.text_range())
            .field("value_text", &self.value_text())
            .finish()
    }
}

impl PartialEq for SyntaxToken<'_> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.tree, other.tree) && self.id == other.id
    }
}

impl Eq for SyntaxToken<'_> {}

impl hash::Hash for SyntaxToken<'_> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        ptr::hash(self.tree, state);
        self.id.hash(state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SyntaxTokenWithParent<'a> {
    pub parent: SyntaxNode<'a>,
    pub tok: SyntaxToken<'a>,
}

impl SyntaxTokenWithParent<'_> {
    pub fn range(&self) -> Option<SourceRange> {
        self.tok.range()
    }
}

impl<'a> std::ops::Deref for SyntaxTokenWithParent<'a> {
    type Target = SyntaxToken<'a>;

    fn deref(&self) -> &Self::Target {
        &self.tok
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyntaxElement<'a> {
    Node(SyntaxNode<'a>),
    Token(SyntaxTokenWithParent<'a>),
}

impl<'a> SyntaxElement<'a> {
    pub fn from_node(node: SyntaxNode<'a>) -> SyntaxElement<'a> {
        SyntaxElement::Node(node)
    }

    pub fn from_token(tok_with_parent: SyntaxTokenWithParent<'a>) -> SyntaxElement<'a> {
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
            SyntaxElement::Token(token) => Some(*token),
            SyntaxElement::Node(_) => None,
        }
    }

    pub fn as_token(&self) -> Option<SyntaxToken<'a>> {
        match self {
            SyntaxElement::Token(token) => Some(token.tok),
            SyntaxElement::Node(_) => None,
        }
    }

    pub fn child_count(&self) -> usize {
        match self {
            SyntaxElement::Node(node) => node.child_count(),
            SyntaxElement::Token(_) => 0,
        }
    }

    pub fn child(&self, idx: usize) -> Option<SyntaxElement<'a>> {
        match self {
            SyntaxElement::Node(node) => node.child(idx),
            SyntaxElement::Token(_) => None,
        }
    }

    pub fn range(&self) -> Option<SourceRange> {
        match self {
            SyntaxElement::Node(node) => node.range(),
            SyntaxElement::Token(token) => token.range(),
        }
    }

    pub fn parent(&self) -> Option<SyntaxNode<'a>> {
        match self {
            SyntaxElement::Node(node) => node.parent(),
            SyntaxElement::Token(token) => Some(token.parent),
        }
    }

    pub fn kind(&self) -> SyntaxElementKind {
        match self {
            SyntaxElement::Node(node) => SyntaxElementKind::Node(node.kind()),
            SyntaxElement::Token(token) => SyntaxElementKind::Token(token.kind()),
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
    fn from(token: SyntaxTokenWithParent<'a>) -> SyntaxElement<'a> {
        SyntaxElement::Token(token)
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

#[derive(Debug, Clone, Copy)]
pub struct SyntaxTrivia<'a> {
    tree: &'a TreeData,
    token: usize,
    idx: usize,
}

impl<'a> SyntaxTrivia<'a> {
    pub fn get_raw_text(&self) -> &str {
        &self.data().raw_text
    }

    pub fn kind(&self) -> TriviaKind {
        self.data().kind
    }

    pub fn range(&self) -> Option<TextRange> {
        self.data().range
    }

    pub fn syntax(&self) -> Option<SyntaxNode<'a>> {
        Some(SyntaxNode { tree: self.tree, id: self.tree.tokens[self.token].parent })
    }

    fn data(&self) -> &OwnedTrivia {
        &self.tree.tokens[self.token].trivia[self.idx]
    }
}

impl PartialEq for SyntaxTrivia<'_> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.tree, other.tree) && self.token == other.token && self.idx == other.idx
    }
}

impl Eq for SyntaxTrivia<'_> {}

impl hash::Hash for SyntaxTrivia<'_> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        ptr::hash(self.tree, state);
        self.token.hash(state);
        self.idx.hash(state);
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxTriviaIter<'a> {
    token: SyntaxToken<'a>,
    start: usize,
    end: usize,
}

impl<'a> Iterator for SyntaxTriviaIter<'a> {
    type Item = SyntaxTrivia<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let idx = self.start;
        self.start += 1;
        Some(SyntaxTrivia { tree: self.token.tree, token: self.token.id, idx })
    }
}

impl DoubleEndedIterator for SyntaxTriviaIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        self.end -= 1;
        Some(SyntaxTrivia { tree: self.token.tree, token: self.token.id, idx: self.end })
    }
}

impl ExactSizeIterator for SyntaxTriviaIter<'_> {
    fn len(&self) -> usize {
        self.end - self.start
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxTriviaWithLocIter<'a> {
    token: SyntaxToken<'a>,
    start: usize,
    end: usize,
}

impl<'a> Iterator for SyntaxTriviaWithLocIter<'a> {
    type Item = (SyntaxTriviaLoc, SyntaxTrivia<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        let idx = self.start;
        self.start += 1;
        trivia_with_loc(self.token, idx)
    }
}

impl DoubleEndedIterator for SyntaxTriviaWithLocIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.start >= self.end {
            return None;
        }
        self.end -= 1;
        trivia_with_loc(self.token, self.end)
    }
}

impl ExactSizeIterator for SyntaxTriviaWithLocIter<'_> {
    fn len(&self) -> usize {
        self.end - self.start
    }
}

fn trivia_with_loc<'a>(
    token: SyntaxToken<'a>,
    idx: usize,
) -> Option<(SyntaxTriviaLoc, SyntaxTrivia<'a>)> {
    let trivia = SyntaxTrivia { tree: token.tree, token: token.id, idx };
    let range = trivia.range()?;
    Some((
        SyntaxTriviaLoc {
            buffer_id: token.tree.buffer_id,
            start: range.start().into(),
            end: range.end().into(),
        },
        trivia,
    ))
}

#[derive(Debug, Clone)]
pub struct SyntaxIdxChildren<'a> {
    parent: SyntaxNode<'a>,
    start_idx: usize,
    end_idx: usize,
}

impl<'a> SyntaxIdxChildren<'a> {
    pub fn new(parent: SyntaxNode<'a>) -> Self {
        SyntaxIdxChildren { parent, start_idx: 0, end_idx: parent.child_count() }
    }
}

impl<'a> Iterator for SyntaxIdxChildren<'a> {
    type Item = (usize, SyntaxElement<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        while self.start_idx < self.end_idx {
            let idx = self.start_idx;
            self.start_idx += 1;
            if let Some(child) = self.parent.child(idx) {
                return Some((idx, child));
            }
        }
        None
    }
}

impl<'a> DoubleEndedIterator for SyntaxIdxChildren<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.start_idx < self.end_idx {
            self.end_idx -= 1;
            if let Some(child) = self.parent.child(self.end_idx) {
                return Some((self.end_idx, child));
            }
        }
        None
    }
}

impl ExactSizeIterator for SyntaxIdxChildren<'_> {
    fn len(&self) -> usize {
        self.end_idx - self.start_idx
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxChildren<'a>(SyntaxIdxChildren<'a>);

impl<'a> SyntaxChildren<'a> {
    pub fn new(parent: SyntaxNode<'a>) -> Self {
        SyntaxChildren(SyntaxIdxChildren::new(parent))
    }
}

impl<'a> Iterator for SyntaxChildren<'a> {
    type Item = SyntaxElement<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, elem)| elem)
    }
}

impl<'a> DoubleEndedIterator for SyntaxChildren<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, elem)| elem)
    }
}

impl ExactSizeIterator for SyntaxChildren<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxAncestors<'a> {
    node: Option<SyntaxNode<'a>>,
}

impl<'a> SyntaxAncestors<'a> {
    pub fn start_from(node: SyntaxNode<'a>) -> Self {
        SyntaxAncestors { node: Some(node) }
    }
}

impl<'a> Iterator for SyntaxAncestors<'a> {
    type Item = SyntaxNode<'a>;

    fn next(&mut self) -> Option<SyntaxNode<'a>> {
        let res = self.node.take()?;
        self.node = res.parent();
        Some(res)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkEvent<T> {
    Enter(T),
    Leave(T),
}

#[derive(Debug, Clone)]
pub struct SyntaxCursor<'a> {
    elem: SyntaxElement<'a>,
    path: Vec<(SyntaxNode<'a>, usize)>,
}

impl<'a> SyntaxCursor<'a> {
    pub fn new(root: SyntaxNode<'a>) -> SyntaxCursor<'a> {
        SyntaxCursor { elem: SyntaxElement::Node(root), path: Vec::with_capacity(16) }
    }

    pub fn to_elem(&self) -> SyntaxElement<'a> {
        self.elem
    }

    pub fn to_node(&self) -> Option<SyntaxNode<'a>> {
        self.elem.as_node()
    }

    pub fn to_tok_with_parent(&self) -> Option<SyntaxTokenWithParent<'a>> {
        self.elem.as_tok_with_parent()
    }

    pub fn to_token(&self) -> Option<SyntaxToken<'a>> {
        self.elem.as_token()
    }

    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    pub fn reset(&mut self, root: SyntaxNode<'a>) {
        self.elem = SyntaxElement::Node(root);
        self.path.clear();
    }

    pub fn reset_to_root(&mut self) {
        if let Some((root, _)) = self.path.first().copied() {
            self.elem = SyntaxElement::Node(root);
            self.path.clear();
        }
    }

    pub fn goto_first_child(&mut self) -> bool {
        let Some((idx, child)) = self.elem.children_with_idx().next() else {
            return false;
        };
        self.path.push((self.to_node().unwrap(), idx));
        self.elem = child;
        true
    }

    pub fn goto_last_child(&mut self) -> bool {
        let Some((idx, child)) = self.elem.children_with_idx().last() else {
            return false;
        };
        self.path.push((self.to_node().unwrap(), idx));
        self.elem = child;
        true
    }

    pub fn goto_parent(&mut self) -> bool {
        let Some((parent, _)) = self.path.pop() else {
            return false;
        };
        self.elem = SyntaxElement::Node(parent);
        true
    }

    pub fn goto_next_sibling(&mut self) -> bool {
        let Some((parent, idx)) = self.path.last_mut() else {
            return false;
        };
        while *idx + 1 < parent.child_count() {
            *idx += 1;
            if let Some(child) = parent.child(*idx) {
                self.elem = child;
                return true;
            }
        }
        false
    }

    pub fn goto_prev_sibling(&mut self) -> bool {
        let Some((parent, idx)) = self.path.last_mut() else {
            return false;
        };
        while *idx > 0 {
            *idx -= 1;
            if let Some(child) = parent.child(*idx) {
                self.elem = child;
                return true;
            }
        }
        false
    }

    pub fn idx(&self) -> Option<usize> {
        self.path.last().map(|(_, idx)| *idx)
    }

    pub fn goto_first_child_after_pos(&mut self, byte: usize) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };
        for (idx, child) in node.children_with_idx() {
            if child.range().is_some_and(|range| range.end() > byte) {
                self.path.push((node, idx));
                self.elem = child;
                return true;
            }
        }
        false
    }

    pub fn goto_last_child_before_pos(&mut self, byte: usize) -> bool {
        let Some(node) = self.to_node() else {
            return false;
        };
        for (idx, child) in node.children_with_idx().rev() {
            if child.range().is_some_and(|range| range.start() < byte) {
                self.path.push((node, idx));
                self.elem = child;
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxNodePreorder<'a> {
    cursor: SyntaxCursor<'a>,
    leaving: bool,
}

impl<'a> SyntaxNodePreorder<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        SyntaxNodePreorder { cursor: SyntaxCursor::new(root), leaving: false }
    }

    pub fn skip_subtree(&mut self) {
        assert!(!self.leaving);
        self.leaving = true;
        self.cursor.goto_parent();
    }
}

impl<'a> Iterator for SyntaxNodePreorder<'a> {
    type Item = WalkEvent<SyntaxNode<'a>>;

    fn next(&mut self) -> Option<WalkEvent<SyntaxNode<'a>>> {
        if self.leaving && self.cursor.is_root() {
            return None;
        }

        let event = if self.leaving {
            WalkEvent::Leave(self.cursor.to_node().unwrap())
        } else {
            WalkEvent::Enter(self.cursor.to_node().unwrap())
        };

        if self.leaving {
            loop {
                if !self.cursor.goto_next_sibling() {
                    self.cursor.goto_parent();
                    break;
                } else if self.cursor.to_node().is_some() {
                    self.leaving = false;
                    break;
                }
            }
        } else if self.cursor.goto_first_child() {
            loop {
                if self.cursor.to_node().is_some() {
                    break;
                } else if !self.cursor.goto_next_sibling() {
                    self.leaving = true;
                    self.cursor.goto_parent();
                    break;
                }
            }
        } else {
            self.leaving = true;
        }

        Some(event)
    }
}

#[derive(Debug, Clone)]
pub struct SyntaxElemPreorder<'a> {
    cursor: SyntaxCursor<'a>,
    leaving: bool,
}

impl<'a> SyntaxElemPreorder<'a> {
    pub fn new(root: SyntaxNode<'a>) -> Self {
        SyntaxElemPreorder { cursor: SyntaxCursor::new(root), leaving: false }
    }

    pub fn skip_subtree(&mut self) {
        assert!(!self.leaving);
        self.leaving = true;
        self.cursor.goto_parent();
    }
}

impl<'a> Iterator for SyntaxElemPreorder<'a> {
    type Item = WalkEvent<SyntaxElement<'a>>;

    fn next(&mut self) -> Option<WalkEvent<SyntaxElement<'a>>> {
        if self.leaving && self.cursor.is_root() {
            return None;
        }

        let event = if self.leaving {
            WalkEvent::Leave(self.cursor.to_elem())
        } else {
            WalkEvent::Enter(self.cursor.to_elem())
        };

        if self.leaving {
            if self.cursor.goto_next_sibling() {
                self.leaving = false;
            } else {
                self.cursor.goto_parent();
            }
        } else if !self.cursor.goto_first_child() {
            self.leaving = true;
        }

        Some(event)
    }
}

fn numeric_prefix(raw: &str) -> &str {
    raw.trim_start().trim_end_matches(|ch: char| ch.is_ascii_alphabetic()).trim_end()
}

fn keyword_texts() -> Vec<&'static str> {
    vec![
        "alias",
        "always",
        "always_comb",
        "always_ff",
        "always_latch",
        "and",
        "assign",
        "automatic",
        "begin",
        "bind",
        "buf",
        "bufif0",
        "bufif1",
        "case",
        "cell",
        "checker",
        "class",
        "cmos",
        "config",
        "default",
        "design",
        "do",
        "edge",
        "else",
        "end",
        "endcase",
        "endclass",
        "endconfig",
        "endfunction",
        "endgenerate",
        "endinterface",
        "endmodule",
        "endpackage",
        "endprimitive",
        "endprogram",
        "endtask",
        "export",
        "for",
        "foreach",
        "forever",
        "function",
        "generate",
        "genvar",
        "if",
        "ifnone",
        "import",
        "include",
        "initial",
        "input",
        "inout",
        "instance",
        "integer",
        "interface",
        "library",
        "localparam",
        "logic",
        "macromodule",
        "module",
        "nand",
        "negedge",
        "nor",
        "not",
        "notif0",
        "notif1",
        "or",
        "output",
        "package",
        "parameter",
        "posedge",
        "primitive",
        "program",
        "pulsestyle_ondetect",
        "pulsestyle_onevent",
        "rcmos",
        "ref",
        "reg",
        "repeat",
        "return",
        "rnmos",
        "rpmos",
        "showcancelled",
        "specparam",
        "task",
        "type",
        "typedef",
        "wait",
        "while",
        "wire",
        "xnor",
        "xor",
    ]
}

fn keyword_kind(text: &str) -> Option<TokenKind> {
    Some(match text {
        "alias" => TokenKind::ALIAS_KEYWORD,
        "always" => TokenKind::ALWAYS_KEYWORD,
        "always_comb" => TokenKind::ALWAYS_COMB_KEYWORD,
        "always_ff" => TokenKind::ALWAYS_FF_KEYWORD,
        "always_latch" => TokenKind::ALWAYS_LATCH_KEYWORD,
        "and" => TokenKind::AND_KEYWORD,
        "assign" => TokenKind::ASSIGN_KEYWORD,
        "automatic" => TokenKind::AUTOMATIC_KEYWORD,
        "begin" => TokenKind::BEGIN_KEYWORD,
        "bind" => TokenKind::BIND_KEYWORD,
        "buf" => TokenKind::BUF_KEYWORD,
        "bufif0" => TokenKind::BUF_IF_0_KEYWORD,
        "bufif1" => TokenKind::BUF_IF_1_KEYWORD,
        "case" => TokenKind::CASE_KEYWORD,
        "cell" => TokenKind::CELL_KEYWORD,
        "checker" => TokenKind::CHECKER_KEYWORD,
        "class" => TokenKind::CLASS_KEYWORD,
        "cmos" => TokenKind::CMOS_KEYWORD,
        "config" => TokenKind::CONFIG_KEYWORD,
        "default" => TokenKind::DEFAULT_KEYWORD,
        "design" => TokenKind::DESIGN_KEYWORD,
        "do" => TokenKind::DO_KEYWORD,
        "edge" => TokenKind::EDGE_KEYWORD,
        "else" => TokenKind::ELSE_KEYWORD,
        "end" => TokenKind::END_KEYWORD,
        "endcase" => TokenKind::END_CASE_KEYWORD,
        "endclass" => TokenKind::END_CLASS_KEYWORD,
        "endconfig" => TokenKind::END_CONFIG_KEYWORD,
        "endfunction" => TokenKind::END_FUNCTION_KEYWORD,
        "endgenerate" => TokenKind::END_GENERATE_KEYWORD,
        "endinterface" => TokenKind::END_INTERFACE_KEYWORD,
        "endmodule" => TokenKind::END_MODULE_KEYWORD,
        "endpackage" => TokenKind::END_PACKAGE_KEYWORD,
        "endprimitive" => TokenKind::END_PRIMITIVE_KEYWORD,
        "endprogram" => TokenKind::END_PROGRAM_KEYWORD,
        "endtask" => TokenKind::END_TASK_KEYWORD,
        "export" => TokenKind::EXPORT_KEYWORD,
        "for" => TokenKind::FOR_KEYWORD,
        "foreach" => TokenKind::FOREACH_KEYWORD,
        "forever" => TokenKind::FOREVER_KEYWORD,
        "function" => TokenKind::FUNCTION_KEYWORD,
        "generate" => TokenKind::GENERATE_KEYWORD,
        "genvar" => TokenKind::GEN_VAR_KEYWORD,
        "if" => TokenKind::IF_KEYWORD,
        "ifnone" => TokenKind::IF_NONE_KEYWORD,
        "import" => TokenKind::IMPORT_KEYWORD,
        "include" => TokenKind::INCLUDE_KEYWORD,
        "initial" => TokenKind::INITIAL_KEYWORD,
        "input" => TokenKind::INPUT_KEYWORD,
        "inout" => TokenKind::IN_OUT_KEYWORD,
        "instance" => TokenKind::INSTANCE_KEYWORD,
        "integer" => TokenKind::INTEGER_KEYWORD,
        "interface" => TokenKind::INTERFACE_KEYWORD,
        "library" => TokenKind::LIBRARY_KEYWORD,
        "localparam" => TokenKind::LOCAL_PARAM_KEYWORD,
        "logic" => TokenKind::LOGIC_KEYWORD,
        "macromodule" => TokenKind::MACROMODULE_KEYWORD,
        "module" => TokenKind::MODULE_KEYWORD,
        "nand" => TokenKind::NAND_KEYWORD,
        "negedge" => TokenKind::NEG_EDGE_KEYWORD,
        "nor" => TokenKind::NOR_KEYWORD,
        "not" => TokenKind::NOT_KEYWORD,
        "notif0" => TokenKind::NOT_IF_0_KEYWORD,
        "notif1" => TokenKind::NOT_IF_1_KEYWORD,
        "or" => TokenKind::OR_KEYWORD,
        "output" => TokenKind::OUTPUT_KEYWORD,
        "package" => TokenKind::PACKAGE_KEYWORD,
        "parameter" => TokenKind::PARAMETER_KEYWORD,
        "posedge" => TokenKind::POS_EDGE_KEYWORD,
        "primitive" => TokenKind::PRIMITIVE_KEYWORD,
        "program" => TokenKind::PROGRAM_KEYWORD,
        "pulsestyle_ondetect" => TokenKind::PULSE_STYLE_ON_DETECT_KEYWORD,
        "pulsestyle_onevent" => TokenKind::PULSE_STYLE_ON_EVENT_KEYWORD,
        "rcmos" => TokenKind::RCMOS_KEYWORD,
        "ref" => TokenKind::REF_KEYWORD,
        "reg" => TokenKind::REG_KEYWORD,
        "repeat" => TokenKind::REPEAT_KEYWORD,
        "return" => TokenKind::RETURN_KEYWORD,
        "rnmos" => TokenKind::RNMOS_KEYWORD,
        "rpmos" => TokenKind::RPMOS_KEYWORD,
        "showcancelled" => TokenKind::SHOW_CANCELLED_KEYWORD,
        "specparam" => TokenKind::SPEC_PARAM_KEYWORD,
        "task" => TokenKind::TASK_KEYWORD,
        "type" => TokenKind::TYPE_KEYWORD,
        "typedef" => TokenKind::TYPEDEF_KEYWORD,
        "wait" => TokenKind::WAIT_KEYWORD,
        "while" => TokenKind::WHILE_KEYWORD,
        "wire" => TokenKind::WIRE_KEYWORD,
        "xnor" => TokenKind::XNOR_KEYWORD,
        "xor" => TokenKind::XOR_KEYWORD,
        _ => return None,
    })
}

impl From<TextRange> for SourceRange {
    fn from(range: TextRange) -> Self {
        SourceRange::new(0, range.start().into(), range.end().into())
    }
}
