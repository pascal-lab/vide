use std::{fmt::Debug, hash::Hash};

pub(crate) use la_arena::{ArenaMap, Idx};
use rustc_hash::FxHashMap;
use syntax::{ast, ast::AstNode};
use triomphe::Arc;
pub(crate) use utils::get::Get;
use utils::get::GetRef;

pub trait IsSrc: PartialEq + Eq + Hash + Copy + Clone + Debug {
    #[inline]
    fn hir<'a, Hir, HirIdx, Arn, SrcMap>(
        self,
        arena: &'a Arc<Arn>,
        src_map: &'a Arc<SrcMap>,
    ) -> &'a Hir
    where
        Arn: GetRef<HirIdx, Output = Hir>,
        SrcMap: Get<Self, Output = HirIdx>,
    {
        let idx = src_map.get(self);
        arena.get(idx)
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
}

impl<Src: IsSrc, Hir> Get<Src> for SourceMap<Src, Hir> {
    type Output = Idx<Hir>;

    fn get_opt(&self, src: Src) -> Option<Self::Output> {
        self.src2hir.get(&src).copied()
    }
}

impl<Src: IsSrc, Hir> Get<Idx<Hir>> for SourceMap<Src, Hir> {
    type Output = Src;

    fn get_opt(&self, idx: Idx<Hir>) -> Option<Self::Output> {
        self.hir2src.get(idx).copied()
    }
}

impl<Src: IsSrc, Hir> Default for SourceMap<Src, Hir> {
    fn default() -> Self {
        SourceMap { src2hir: FxHashMap::default(), hir2src: ArenaMap::default() }
    }
}

pub(crate) macro impl_source_map_idx {
    ($datas:ident => $($fld:ident[$src:ty, $hir_id:ty]),+ $(,)? ) => {
        $(
            impl $crate::source_map::Get<$src> for $datas {
                type Output = $hir_id;
                fn get_opt(&self, src: $src) -> Option<Self::Output> {
                    self.$fld.get_opt(src)
                }
            }

            impl $crate::source_map::Get<$hir_id> for $datas {
                type Output = $src;
                fn get_opt(&self, idx: $hir_id) -> Option<Self::Output> {
                    self.$fld.get_opt(idx)
                }
            }
        )+
    }
}

pub trait ToAstNode<'a, Output: AstNode<'a>> {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<Output>;
}

pub(crate) macro define_src {
    ($name:ident(ast::$ty:ident)) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub struct $name(pub syntax::ptr::SyntaxNodePtr);

        impl $crate::source_map::IsSrc for $name {}

        impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
            fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::$ty<'a>> {
                let mut node = self.0.to_node(tree)?;
                while !<ast::$ty<'a> as syntax::ast::AstNode>::can_cast(node.kind()) && node.child_count() == 1 {
                    node = node.child_node(0).unwrap();
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
    },

    ($name:ident($(ast::$ty:ident),*)$(,)?) => {
        #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
        pub enum $name {
            $(
                $ty(syntax::ptr::SyntaxNodePtr),
            )*
        }

        impl $crate::source_map::IsSrc for $name {}

        $(
            impl<'a> $crate::source_map::ToAstNode<'a, ast::$ty<'a>> for $name {
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
    }
}
