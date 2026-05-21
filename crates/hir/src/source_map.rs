use std::{fmt::Debug, hash::Hash};

pub(crate) use la_arena::{ArenaMap, Idx};
use rustc_hash::FxHashMap;
use syntax::{SyntaxKind, TokenKind, ast::AstNode};
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

    fn range(&self) -> TextRange;
}

pub trait IsNamedSrc: IsSrc {
    fn name_kind(&self) -> Option<TokenKind>;

    fn name_range(&self) -> Option<TextRange>;

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
            fn range(&self) -> utils::text_edit::TextRange {
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
            fn range(&self) -> utils::text_edit::TextRange {
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

            fn range(&self) -> utils::text_edit::TextRange {
                self.node.range()
            }
        }

        impl $crate::source_map::IsNamedSrc for $name {
            fn name_kind(&self) -> Option<syntax::TokenKind> {
                self.name.map(|name| name.kind())
            }

            fn name_range(&self) -> Option<utils::text_edit::TextRange> {
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

            fn range(&self) -> utils::text_edit::TextRange {
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

            fn name_range(&self) -> Option<utils::text_edit::TextRange> {
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

            fn range(&self) -> utils::text_edit::TextRange {
                self.node.range()
            }
        }

        impl $crate::source_map::IsNamedSrc for $name {
            fn name_kind(&self) -> Option<syntax::TokenKind> {
                self.name.map(|name| name.kind())
            }

            fn name_range(&self) -> Option<utils::text_edit::TextRange> {
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
