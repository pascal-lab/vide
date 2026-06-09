use std::{fmt::Debug, hash::Hash};

use ::preproc::source::{PreprocSourceId, SourceRange as PreprocSourceRange};
pub(crate) use la_arena::{ArenaMap, Idx};
use rustc_hash::FxHashMap;
use syntax::{
    PreprocessorTraceTokenProvenance, SourceBufferRange, SyntaxElement, SyntaxKind, SyntaxNode,
    SyntaxToken, SyntaxTokenWithParent, TokenKind, WalkEvent, ast::AstNode,
    has_text_range::HasTextRange,
};
pub(crate) use utils::get::Get;
use utils::{
    get::GetRef,
    text_edit::{TextRange, TextSize},
};

use crate::preproc::{
    MacroArgumentTokenIdentity, MacroBodyTokenIdentity, MacroOperationTokenIdentity,
};

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

    /// Returns the full syntactic extent of the mapped expanded AST node.
    ///
    /// Use this for containment, folding, diagnostics, and operations that act
    /// on the whole construct rather than just its defining identifier.
    ///
    /// This is a HIR/source-map coordinate. Editor-facing ranges must resolve a
    /// presentation anchor instead of treating this range as original source.
    fn expanded_range(&self) -> TextRange;
}

pub trait IsNamedSrc: IsSrc {
    fn name_kind(&self) -> Option<TokenKind>;

    /// Returns the expanded token range that names this source node, when it
    /// has one.
    ///
    /// This is a HIR/source-map coordinate. Editor-facing focus ranges must
    /// resolve a presentation anchor instead of treating this range as original
    /// source.
    fn expanded_name_range(&self) -> Option<TextRange>;

    /// Returns the expanded symbol focus range when present, otherwise the full
    /// expanded range.
    fn expanded_name_or_full_range(&self) -> TextRange {
        self.expanded_name_range().unwrap_or_else(|| self.expanded_range())
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceMap<Src: IsSrc, Hir> {
    src2hir: FxHashMap<Src, Idx<Hir>>,
    hir2src: ArenaMap<Idx<Hir>, Src>,
    hir2presentation: ArenaMap<Idx<Hir>, SourcePresentation>,
}

impl<Src: IsSrc, Hir> SourceMap<Src, Hir> {
    pub fn insert(&mut self, src: Src, idx: Idx<Hir>) {
        self.insert_with_presentation(src, idx, SourcePresentation::direct(src.expanded_range()));
    }

    pub fn insert_with_presentation(
        &mut self,
        src: Src,
        idx: Idx<Hir>,
        presentation: SourcePresentation,
    ) {
        self.src2hir.insert(src, idx);
        self.hir2src.insert(idx, src);
        self.hir2presentation.insert(idx, presentation);
    }

    pub fn shrink_to_fit(&mut self) {
        self.src2hir.shrink_to_fit();
        self.hir2src.shrink_to_fit();
        self.hir2presentation.shrink_to_fit();
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

    #[inline]
    pub fn hir_to_presentation(&self, idx: Idx<Hir>) -> Option<&SourcePresentation> {
        self.hir2presentation.get(idx)
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
        SourceMap {
            src2hir: FxHashMap::default(),
            hir2src: ArenaMap::default(),
            hir2presentation: ArenaMap::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePresentation {
    pub full: SourcePresentationAnchor,
    pub name: Option<SourcePresentationAnchor>,
}

impl SourcePresentation {
    pub fn direct(range: TextRange) -> Self {
        Self { full: SourcePresentationAnchor::Direct(range), name: None }
    }

    pub(crate) fn from_node_and_name(node: SyntaxNode<'_>, name: Option<SyntaxToken<'_>>) -> Self {
        let full = SourcePresentationAnchor::from_node(node);
        let name = name
            .and_then(|name| root_token_in(node, name).map(SourcePresentationAnchor::from_token));
        Self { full, name }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourcePresentationAnchor {
    Direct(TextRange),
    Source(PreprocSourceRange),
    MacroBody(MacroBodyTokenIdentity),
    MacroArgument(MacroArgumentTokenIdentity),
    MacroOperation(MacroOperationTokenIdentity),
    Unavailable,
}

impl SourcePresentationAnchor {
    fn from_node(node: SyntaxNode<'_>) -> Self {
        let Some(direct_range) = node.text_range() else {
            return Self::Unavailable;
        };
        let mut saw_token = false;
        let mut direct = true;
        let mut source_range = None;
        let mut macro_anchor = None;

        for event in node.elem_preorder() {
            let WalkEvent::Enter(SyntaxElement::Token(token)) = event else {
                continue;
            };
            saw_token = true;
            let anchor = Self::from_token(token);
            match anchor {
                Self::Direct(_) => {
                    if source_range.is_some() || macro_anchor.is_some() {
                        return Self::Unavailable;
                    }
                }
                Self::Source(range) => {
                    direct = false;
                    if macro_anchor.is_some() {
                        return Self::Unavailable;
                    }
                    source_range = match source_range {
                        Some(existing) => merge_source_ranges(existing, range),
                        None => Some(range),
                    };
                    if source_range.is_none() {
                        return Self::Unavailable;
                    }
                }
                Self::MacroBody(_) | Self::MacroArgument(_) | Self::MacroOperation(_) => {
                    direct = false;
                    if source_range.is_some() {
                        return Self::Unavailable;
                    }
                    macro_anchor = match macro_anchor {
                        Some(existing) if existing == anchor => Some(existing),
                        Some(_) => return Self::Unavailable,
                        None => Some(anchor),
                    };
                }
                Self::Unavailable => return Self::Unavailable,
            }
        }

        if !saw_token || direct {
            return Self::Direct(direct_range);
        }
        source_range.map(Self::Source).or(macro_anchor).unwrap_or(Self::Unavailable)
    }

    fn from_token(token: SyntaxTokenWithParent<'_>) -> Self {
        match token.preprocessor_trace_provenance() {
            PreprocessorTraceTokenProvenance::Source { token_range } => {
                source_range_from_trace(token_range).map(Self::Source).unwrap_or(Self::Unavailable)
            }
            PreprocessorTraceTokenProvenance::MacroBody { identity, .. } => {
                Self::MacroBody(identity.into())
            }
            PreprocessorTraceTokenProvenance::MacroArgument { identity, .. } => {
                Self::MacroArgument(identity.into())
            }
            PreprocessorTraceTokenProvenance::TokenPaste { identity }
            | PreprocessorTraceTokenProvenance::Stringification { identity } => {
                Self::MacroOperation(identity.into())
            }
            PreprocessorTraceTokenProvenance::Builtin { .. }
            | PreprocessorTraceTokenProvenance::Unavailable => Self::Unavailable,
        }
    }
}

fn source_range_from_trace(range: SourceBufferRange) -> Option<PreprocSourceRange> {
    Some(PreprocSourceRange {
        source: PreprocSourceId::from(range.buffer_id),
        range: TextRange::new(
            TextSize::from(u32::try_from(range.range.start).ok()?),
            TextSize::from(u32::try_from(range.range.end).ok()?),
        ),
    })
}

fn merge_source_ranges(
    left: PreprocSourceRange,
    right: PreprocSourceRange,
) -> Option<PreprocSourceRange> {
    (left.source == right.source).then(|| PreprocSourceRange {
        source: left.source,
        range: TextRange::new(
            left.range.start().min(right.range.start()),
            left.range.end().max(right.range.end()),
        ),
    })
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
    pub(crate) ast: Ast,
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

    fn source_presentation(ast: &SourceAst<Ast>) -> SourcePresentation {
        SourcePresentation::from_node_and_name(ast.ast.syntax(), None)
    }

    fn from_source_ast_with_presentation(ast: SourceAst<Ast>) -> (Self, SourcePresentation)
    where
        Self: Sized,
    {
        let presentation = Self::source_presentation(&ast);
        (Self::from_source_ast(ast), presentation)
    }
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

#[macro_export]
macro_rules! define_src {
    ($name:ident(ast::$ty:ident)) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub struct $name(pub syntax::ptr::SyntaxNodePtr);

        impl $crate::source_map::IsSrc for $name {
            #[inline]
            fn kind(&self) -> syntax::SyntaxKind {
                self.0.kind()
            }

            #[inline]
            fn expanded_range(&self) -> utils::text_edit::TextRange {
                self.0.range()
            }
        }

        impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
            #[inline]
            fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::$ty<'a>> {
                let mut node = self.0.to_node(tree)?;
                while !<ast::$ty<'a> as syntax::ast::AstNode>::can_cast(node.kind()) {
                    node = node.children().find_map(|elem| elem.as_node())?;
                }
                <ast::$ty<'a> as syntax::ast::AstNode>::cast(node)
            }
        }

        impl From<ast::$ty<'_>> for $name {
            fn from(node: ast::$ty<'_>) -> Self {
                Self(syntax::slang_ext::AstNodeExt::to_ptr(&node))
            }
        }

        impl<'a> $crate::source_map::FromSourceAst<'a, ast::$ty<'a>> for $name {
            fn from_source_ast(node: $crate::source_map::SourceAst<ast::$ty<'a>>) -> Self {
                Self(syntax::slang_ext::AstNodeExt::to_ptr(&node.into_inner()))
            }
        }

        impl From<$name> for syntax::ptr::SyntaxNodePtr {
            fn from(src: $name) -> Self {
                src.0
            }
        }
    };

    ($name:ident($(ast::$ty:ident),*)$(,)?) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub enum $name {
            $(
                $ty(syntax::ptr::SyntaxNodePtr),
            )*
        }

        impl $crate::source_map::IsSrc for $name {
            #[inline]
            fn kind(&self) -> syntax::SyntaxKind {
                match self {
                    $(
                        $name::$ty(ptr) => ptr.kind(),
                    )*
                }
            }

            #[inline]
            fn expanded_range(&self) -> utils::text_edit::TextRange {
                match self {
                    $(
                        $name::$ty(ptr) => ptr.range(),
                    )*
                }
            }
        }

        $(
            impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
                #[inline]
                fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::$ty<'a>> {
                    match self {
                        $name::$ty(ptr) => syntax::ast::AstNode::cast(ptr.to_node(tree)?),
                        _ => None,
                    }
                }
            }
        )*

        $(
            impl From<ast::$ty<'_>> for $name {
                fn from(node: ast::$ty<'_>) -> Self {
                    Self::$ty(syntax::slang_ext::AstNodeExt::to_ptr(&node))
                }
            }

            impl<'a> $crate::source_map::FromSourceAst<'a, ast::$ty<'a>> for $name {
                fn from_source_ast(node: $crate::source_map::SourceAst<ast::$ty<'a>>) -> Self {
                    Self::$ty(syntax::slang_ext::AstNodeExt::to_ptr(&node.into_inner()))
                }
            }
        )*
    };
}

#[macro_export]
macro_rules! define_src_with_name {
    ($name:ident(ast::$ty:ident)) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub struct $name {
            pub node: syntax::ptr::SyntaxNodePtr,
            pub name: Option<syntax::ptr::SyntaxTokenPtr>,
        }

        impl $crate::source_map::IsSrc for $name {
            fn kind(&self) -> syntax::SyntaxKind {
                self.node.kind()
            }

            fn expanded_range(&self) -> utils::text_edit::TextRange {
                self.node.range()
            }
        }

        impl $crate::source_map::IsNamedSrc for $name {
            fn name_kind(&self) -> Option<syntax::TokenKind> {
                self.name.map(|name| name.kind())
            }

            fn expanded_name_range(&self) -> Option<utils::text_edit::TextRange> {
                self.name.map(|name| name.range())
            }
        }

        impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
            fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::$ty<'a>> {
                let mut node = self.node.to_node(tree)?;
                while !<ast::$ty<'a> as syntax::ast::AstNode>::can_cast(node.kind()) {
                    node = node.children().find_map(|elem| elem.as_node())?;
                }
                <ast::$ty<'a> as syntax::ast::AstNode>::cast(node)
            }
        }

        impl From<ast::$ty<'_>> for $name {
            fn from(node: ast::$ty<'_>) -> Self {
                let syntax = syntax::ast::AstNode::syntax(&node);
                Self {
                    node: syntax::slang_ext::AstNodeExt::to_ptr(&node),
                    name: <ast::$ty<'_> as syntax::has_name::HasName<'_>>::name(&node)
                        .map(|name| syntax::ptr::SyntaxTokenPtr::from_token_in(syntax, name)),
                }
            }
        }

        impl<'a> $crate::source_map::FromSourceAst<'a, ast::$ty<'a>> for $name {
            fn from_source_ast(node: $crate::source_map::SourceAst<ast::$ty<'a>>) -> Self {
                let node = node.into_inner();
                let syntax = syntax::ast::AstNode::syntax(&node);
                Self {
                    node: syntax::slang_ext::AstNodeExt::to_ptr(&node),
                    name: <ast::$ty<'a> as syntax::has_name::HasName<'a>>::name(&node)
                        .and_then(|name| {
                            $crate::source_map::root_token_in(syntax, name)
                                .map(syntax::ptr::SyntaxTokenPtr::from_token)
                        }),
                }
            }

            fn source_presentation(
                node: &$crate::source_map::SourceAst<ast::$ty<'a>>,
            ) -> $crate::source_map::SourcePresentation {
                $crate::source_map::SourcePresentation::from_node_and_name(
                    syntax::ast::AstNode::syntax(&node.ast),
                    <ast::$ty<'a> as syntax::has_name::HasName<'a>>::name(&node.ast),
                )
            }
        }

        impl From<$name> for syntax::ptr::SyntaxNodePtr {
            fn from(src: $name) -> Self {
                src.node
            }
        }

        impl From<$name> for Option<syntax::ptr::SyntaxTokenPtr> {
            fn from(src: $name) -> Self {
                src.name
            }
        }
    };

    ($name:ident($(ast::$ty:ident),*)$(,)?) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub enum $name {
            $(
                $ty {
                    node: syntax::ptr::SyntaxNodePtr,
                    name: Option<syntax::ptr::SyntaxTokenPtr>,
                },
            )*
        }

        impl $crate::source_map::IsSrc for $name {
            fn kind(&self) -> syntax::SyntaxKind {
                match self {
                    $(
                        $name::$ty { node, .. } => node.kind(),
                    )*
                }
            }

            fn expanded_range(&self) -> utils::text_edit::TextRange {
                match self {
                    $(
                        $name::$ty { node, .. } => node.range(),
                    )*
                }
            }
        }

        impl $crate::source_map::IsNamedSrc for $name {
            fn name_kind(&self) -> Option<syntax::TokenKind> {
                match self {
                    $(
                        $name::$ty { name, .. } => name.map(|name| name.kind()),
                    )*
                }
            }

            fn expanded_name_range(&self) -> Option<utils::text_edit::TextRange> {
                match self {
                    $(
                        $name::$ty { name, .. } => name.map(|name| name.range()),
                    )*
                }
            }
        }

        $(
            impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
                fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::$ty<'a>> {
                    match self {
                        $name::$ty { node, .. } => {
                            let mut node = node.to_node(tree)?;
                            while !<ast::$ty<'a> as syntax::ast::AstNode>::can_cast(node.kind()) && node.child_count() == 1 {
                                node = node.child_node(0)?;
                            }
                            <ast::$ty<'a> as syntax::ast::AstNode>::cast(node)
                        }
                        _ => None,
                    }
                }
            }
        )*

        $(
            impl From<ast::$ty<'_>> for $name {
                fn from(node: ast::$ty<'_>) -> Self {
                    let syntax = syntax::ast::AstNode::syntax(&node);
                    Self::$ty {
                        node: syntax::slang_ext::AstNodeExt::to_ptr(&node),
                        name: <ast::$ty<'_> as syntax::has_name::HasName<'_>>::name(&node)
                            .map(|name| syntax::ptr::SyntaxTokenPtr::from_token_in(syntax, name)),
                    }
                }
            }

            impl<'a> $crate::source_map::FromSourceAst<'a, ast::$ty<'a>> for $name {
                fn from_source_ast(node: $crate::source_map::SourceAst<ast::$ty<'a>>) -> Self {
                    let node = node.into_inner();
                    let syntax = syntax::ast::AstNode::syntax(&node);
                    Self::$ty {
                        node: syntax::slang_ext::AstNodeExt::to_ptr(&node),
                        name: <ast::$ty<'a> as syntax::has_name::HasName<'a>>::name(&node)
                            .and_then(|name| {
                                $crate::source_map::root_token_in(syntax, name)
                                    .map(syntax::ptr::SyntaxTokenPtr::from_token)
                            }),
                    }
                }

                fn source_presentation(
                    node: &$crate::source_map::SourceAst<ast::$ty<'a>>,
                ) -> $crate::source_map::SourcePresentation {
                    $crate::source_map::SourcePresentation::from_node_and_name(
                        syntax::ast::AstNode::syntax(&node.ast),
                        <ast::$ty<'a> as syntax::has_name::HasName<'a>>::name(&node.ast),
                    )
                }
            }
        )*

        impl From<$name> for syntax::ptr::SyntaxNodePtr {
            fn from(src: $name) -> Self {
                match src {
                    $(
                        $name::$ty { node, .. } => node,
                    )*
                }
            }
        }

        impl From<$name> for Option<syntax::ptr::SyntaxTokenPtr> {
            fn from(src: $name) -> Self {
                match src {
                    $(
                        $name::$ty { name, .. } => name,
                    )*
                }
            }
        }
    };
}

#[macro_export]
macro_rules! define_src_with_name_and_token {
    ($name:ident(ast:: $ty:ident, $token:ident : $token_getter:ident, $range_getter:ident)) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub struct $name {
            pub node: syntax::ptr::SyntaxNodePtr,
            pub name: Option<syntax::ptr::SyntaxTokenPtr>,
            $token: Option<syntax::ptr::SyntaxTokenPtr>,
        }

        impl $name {
            pub fn $range_getter(&self) -> Option<utils::text_edit::TextRange> {
                self.$token.map(|token| token.range())
            }
        }

        impl $crate::source_map::IsSrc for $name {
            fn kind(&self) -> syntax::SyntaxKind {
                self.node.kind()
            }

            fn expanded_range(&self) -> utils::text_edit::TextRange {
                self.node.range()
            }
        }

        impl $crate::source_map::IsNamedSrc for $name {
            fn name_kind(&self) -> Option<syntax::TokenKind> {
                self.name.map(|name| name.kind())
            }

            fn expanded_name_range(&self) -> Option<utils::text_edit::TextRange> {
                self.name.map(|name| name.range())
            }
        }

        impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
            fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::$ty<'a>> {
                let mut node = self.node.to_node(tree)?;
                while !<ast::$ty<'a> as syntax::ast::AstNode>::can_cast(node.kind()) {
                    node = node.children().find_map(|elem| elem.as_node())?;
                }
                <ast::$ty<'a> as syntax::ast::AstNode>::cast(node)
            }
        }

        impl From<ast::$ty<'_>> for $name {
            fn from(node: ast::$ty<'_>) -> Self {
                let syntax = syntax::ast::AstNode::syntax(&node);
                Self {
                    node: syntax::slang_ext::AstNodeExt::to_ptr(&node),
                    name: <ast::$ty<'_> as syntax::has_name::HasName<'_>>::name(&node)
                        .map(|name| syntax::ptr::SyntaxTokenPtr::from_token_in(syntax, name)),
                    $token: node
                        .$token_getter()
                        .map(|token| syntax::ptr::SyntaxTokenPtr::from_token_in(syntax, token)),
                }
            }
        }

        impl<'a> $crate::source_map::FromSourceAst<'a, ast::$ty<'a>> for $name {
            fn from_source_ast(node: $crate::source_map::SourceAst<ast::$ty<'a>>) -> Self {
                let node = node.into_inner();
                let syntax = syntax::ast::AstNode::syntax(&node);
                Self {
                    node: syntax::slang_ext::AstNodeExt::to_ptr(&node),
                    name: <ast::$ty<'a> as syntax::has_name::HasName<'a>>::name(&node).and_then(
                        |name| {
                            $crate::source_map::root_token_in(syntax, name)
                                .map(syntax::ptr::SyntaxTokenPtr::from_token)
                        },
                    ),
                    $token: node.$token_getter().and_then(|token| {
                        $crate::source_map::root_token_in(syntax, token)
                            .map(syntax::ptr::SyntaxTokenPtr::from_token)
                    }),
                }
            }

            fn source_presentation(
                node: &$crate::source_map::SourceAst<ast::$ty<'a>>,
            ) -> $crate::source_map::SourcePresentation {
                $crate::source_map::SourcePresentation::from_node_and_name(
                    syntax::ast::AstNode::syntax(&node.ast),
                    <ast::$ty<'a> as syntax::has_name::HasName<'a>>::name(&node.ast),
                )
            }
        }

        impl From<$name> for syntax::ptr::SyntaxNodePtr {
            fn from(src: $name) -> Self {
                src.node
            }
        }

        impl From<$name> for Option<syntax::ptr::SyntaxTokenPtr> {
            fn from(src: $name) -> Self {
                src.name
            }
        }
    };
}
