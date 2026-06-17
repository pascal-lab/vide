use std::{fmt::Debug, hash::Hash, marker::PhantomData};

pub(crate) use la_arena::{ArenaMap, Idx};
use rustc_hash::FxHashMap;
use syntax::{
    SyntaxKind, SyntaxNode, SyntaxToken, SyntaxTokenWithParent, TokenKind,
    ast::AstNode,
    has_text_range::HasTextRange,
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
};
pub(crate) use utils::get::Get;
use utils::{get::GetRef, text_edit::TextRange};

pub trait IsSrc: PartialEq + Eq + Hash + Copy + Clone + Debug {
    #[inline]
    fn hir<'a, Hir, HirIdx, Arn, SrcMap>(
        self,
        arena: &'a impl AsRef<Arn>,
        src_map: &'a impl AsRef<SrcMap>,
    ) -> Option<&'a Hir>
    where
        Arn: GetRef<HirIdx, Output = Hir> + 'a,
        SrcMap: Get<Self, Output = Option<HirIdx>> + 'a,
    {
        let idx = src_map.as_ref().get(self)?;
        Some(arena.as_ref().get(idx))
    }

    fn kind(&self) -> SyntaxKind;

    /// Returns the full syntactic extent of the mapped AST node.
    ///
    /// Use this for containment, folding, diagnostics, and operations that act
    /// on the whole construct rather than just its defining identifier.
    fn range(&self) -> TextRange;
}

pub trait IsNamedSrc: IsSrc {
    fn name_kind(&self) -> Option<TokenKind>;

    /// Returns the token range that names this source node, when it has one.
    ///
    /// Use this for symbol focus ranges such as navigation targets, document
    /// symbol selections, rename/reference origins, and semantic tokens.
    fn name_range(&self) -> Option<TextRange>;

    /// Returns the symbol focus range when present, otherwise the full range.
    fn name_or_full_range(&self) -> TextRange {
        self.name_range().unwrap_or_else(|| self.range())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceMap<Src: IsSrc, Hir> {
    src2hir: FxHashMap<Src, Idx<Hir>>,
    hir2src: ArenaMap<Idx<Hir>, Src>,
}

impl<Src: IsSrc, Hir> SourceMap<Src, Hir> {
    pub fn insert(&mut self, src: Src, idx: Idx<Hir>) {
        self.src2hir.insert(src, idx);
        self.hir2src.insert(idx, src);
    }

    pub fn shrink_to_fit(&mut self) {
        self.src2hir.shrink_to_fit();
        self.hir2src.shrink_to_fit();
    }

    pub fn iter(&self) -> impl Iterator<Item = (Idx<Hir>, &Src)> {
        self.hir2src.iter()
    }

    #[inline]
    pub fn src_to_hir(&self, src: Src) -> Option<Idx<Hir>> {
        self.src2hir.get(&src).copied()
    }

    #[inline]
    pub fn hir_to_src(&self, idx: Idx<Hir>) -> Option<Src> {
        self.hir2src.get(idx).copied()
    }
}

impl<Src: IsSrc, Hir> Get<Src> for SourceMap<Src, Hir> {
    type Output = Option<Idx<Hir>>;

    fn get(&self, src: Src) -> Self::Output {
        self.src_to_hir(src)
    }
}

impl<Src: IsSrc, Hir> Get<Idx<Hir>> for SourceMap<Src, Hir> {
    type Output = Option<Src>;

    fn get(&self, idx: Idx<Hir>) -> Self::Output {
        self.hir_to_src(idx)
    }
}

impl<Src: IsSrc, Hir> Default for SourceMap<Src, Hir> {
    fn default() -> Self {
        SourceMap { src2hir: FxHashMap::default(), hir2src: ArenaMap::default() }
    }
}

pub trait ToAstNode<'a, Output: AstNode<'a>> {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<Output>;
}

/// AST node that is valid as an IDE source-map location in the parsed root
/// file.
///
/// Slang can expose semantic AST nodes that originate from include or macro
/// expansion. Those nodes are still valid input for HIR lowering, but they do
/// not have a stable text range in the root buffer, so they must not become
/// source-map keys. Use `SourceAst::new` at the HIR allocation/source-map
/// boundary when HIR should still be allocated but the source-map entry may be
/// absent.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SourceAst<Ast> {
    ast: Ast,
}

impl<'a, Ast> SourceAst<Ast>
where
    Ast: AstNode<'a>,
{
    /// Returns `None` when the AST node has no root-buffer text range.
    ///
    /// Callers should treat that as "no navigable source location", not as a
    /// lowering failure.
    pub(crate) fn new(ast: Ast) -> Option<Self> {
        ast.syntax().text_range()?;
        Some(Self { ast })
    }

    pub(crate) fn into_inner(self) -> Ast {
        self.ast
    }
}

/// Conversion from a root-buffer AST node into a source-map key.
///
/// `alloc_idx_and_src!` depends on this trait instead of plain `From<ast::...>`
/// so adding a new source-map entry point requires an explicit implementation
/// that is checked by `cargo check`. Keep ordinary `From<ast::...>` impls for
/// lookup paths that already operate on AST nodes under the cursor in the root
/// file.
pub(crate) trait FromSourceAst<'a, Ast: AstNode<'a>> {
    fn from_source_ast(ast: SourceAst<Ast>) -> Self;
}

/// Attach a bare token returned by generated AST accessors to a root-buffer
/// context.
///
/// Use this inside `FromSourceAst` implementations for optional focus tokens
/// such as names or keywords. A token from macro/include expansion is not a
/// valid root-buffer focus range, so callers should leave that field as `None`
/// while still keeping the enclosing source-map node.
pub(crate) fn root_token_in<'a>(
    context: SyntaxNode<'a>,
    token: SyntaxToken<'a>,
) -> Option<SyntaxTokenWithParent<'a>> {
    let token = SyntaxTokenWithParent { parent: context, tok: token };
    token.text_range()?;
    Some(token)
}

pub(crate) fn ast_node_from_ptr<'a, Ast>(
    ptr: SyntaxNodePtr,
    tree: &'a syntax::SyntaxTree,
) -> Option<Ast>
where
    Ast: AstNode<'a>,
{
    let mut node = ptr.to_node(tree)?;
    while !Ast::can_cast(node.kind()) {
        node = node.children().find_map(|elem| elem.as_node())?;
    }
    Ast::cast(node)
}

pub(crate) fn exact_ast_node_from_ptr<'a, Ast>(
    ptr: SyntaxNodePtr,
    tree: &'a syntax::SyntaxTree,
) -> Option<Ast>
where
    Ast: AstNode<'a>,
{
    Ast::cast(ptr.to_node(tree)?)
}

pub(crate) fn wrapped_ast_node_from_ptr<'a, Ast>(
    ptr: SyntaxNodePtr,
    tree: &'a syntax::SyntaxTree,
) -> Option<Ast>
where
    Ast: AstNode<'a>,
{
    let mut node = ptr.to_node(tree)?;
    while !Ast::can_cast(node.kind()) && node.child_count() == 1 {
        node = node.child_node(0)?;
    }
    Ast::cast(node)
}

pub trait AstKind: Debug + PartialEq + Eq + Hash + Copy + Clone + 'static {
    type Node<'a>: AstNode<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct AstId<Kind: AstKind>(pub SyntaxNodePtr, PhantomData<fn() -> Kind>);

impl<Kind: AstKind> AstId<Kind> {
    #[inline]
    pub fn new(node: SyntaxNodePtr) -> Self {
        Self(node, PhantomData)
    }

    #[inline]
    pub fn from_ast<'a>(node: Kind::Node<'a>) -> Self {
        Self::new(syntax::slang_ext::AstNodeExt::to_ptr(&node))
    }

    #[inline]
    pub(crate) fn from_source_ast<'a>(node: SourceAst<Kind::Node<'a>>) -> Self {
        Self::from_ast(node.into_inner())
    }

    #[inline]
    pub fn ptr(self) -> SyntaxNodePtr {
        self.0
    }
}

impl<Kind: AstKind> IsSrc for AstId<Kind> {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.0.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.0.range()
    }
}

impl<'a, Kind: AstKind> ToAstNode<'a, Kind::Node<'a>> for AstId<Kind> {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<Kind::Node<'a>> {
        ast_node_from_ptr(self.0, tree)
    }
}

impl<'a, Kind: AstKind> FromSourceAst<'a, Kind::Node<'a>> for AstId<Kind> {
    fn from_source_ast(node: SourceAst<Kind::Node<'a>>) -> Self {
        Self::from_source_ast(node)
    }
}

impl<Kind: AstKind> From<AstId<Kind>> for SyntaxNodePtr {
    fn from(src: AstId<Kind>) -> Self {
        src.ptr()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct NamedAstId<Kind: AstKind> {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
    _kind: PhantomData<fn() -> Kind>,
}

impl<Kind: AstKind> NamedAstId<Kind> {
    #[inline]
    pub fn new(node: SyntaxNodePtr, name: Option<SyntaxTokenPtr>) -> Self {
        Self { node, name, _kind: PhantomData }
    }

    #[inline]
    pub fn from_ast<'a>(node: Kind::Node<'a>) -> Self
    where
        Kind::Node<'a>: syntax::has_name::HasName<'a>,
    {
        let syntax = node.syntax();
        Self::new(
            syntax::slang_ext::AstNodeExt::to_ptr(&node),
            <Kind::Node<'a> as syntax::has_name::HasName<'a>>::name(&node)
                .map(|name| SyntaxTokenPtr::from_token_in(syntax, name)),
        )
    }

    #[inline]
    pub(crate) fn from_source_ast<'a>(node: SourceAst<Kind::Node<'a>>) -> Self
    where
        Kind::Node<'a>: syntax::has_name::HasName<'a>,
    {
        let node = node.into_inner();
        let syntax = node.syntax();
        Self::new(
            syntax::slang_ext::AstNodeExt::to_ptr(&node),
            <Kind::Node<'a> as syntax::has_name::HasName<'a>>::name(&node)
                .and_then(|name| root_token_in(syntax, name).map(SyntaxTokenPtr::from_token)),
        )
    }

    #[inline]
    pub fn ast_id(self) -> AstId<Kind> {
        AstId::new(self.node)
    }
}

impl<Kind: AstKind> IsSrc for NamedAstId<Kind> {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl<Kind: AstKind> IsNamedSrc for NamedAstId<Kind> {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a, Kind: AstKind> ToAstNode<'a, Kind::Node<'a>> for NamedAstId<Kind> {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<Kind::Node<'a>> {
        ast_node_from_ptr(self.node, tree)
    }
}

impl<'a, Kind> FromSourceAst<'a, Kind::Node<'a>> for NamedAstId<Kind>
where
    Kind: AstKind,
    Kind::Node<'a>: syntax::has_name::HasName<'a>,
{
    fn from_source_ast(node: SourceAst<Kind::Node<'a>>) -> Self {
        Self::from_source_ast(node)
    }
}

impl<Kind: AstKind> From<NamedAstId<Kind>> for SyntaxNodePtr {
    fn from(src: NamedAstId<Kind>) -> Self {
        src.node
    }
}

impl<Kind: AstKind> From<NamedAstId<Kind>> for Option<SyntaxTokenPtr> {
    fn from(src: NamedAstId<Kind>) -> Self {
        src.name
    }
}
