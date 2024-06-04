use std::{fmt::Debug, hash::Hash, ops::Index};

use la_arena::{ArenaMap, Idx};
use rustc_hash::FxHashMap;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone + Debug,
{
    pub src2hir: FxHashMap<Src, Idx<Hir>>,
    pub hir2src: ArenaMap<Idx<Hir>, Src>,
}

impl<Src, Hir> SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone + Debug,
{
    pub fn insert(&mut self, src: Src, idx: Idx<Hir>) {
        self.src2hir.insert(src.clone(), idx);
        self.hir2src.insert(idx, src);
    }

    pub fn get_idx(&self, src: &Src) -> Option<&Idx<Hir>> {
        self.src2hir.get(src)
    }

    pub fn get_src(&self, idx: Idx<Hir>) -> Option<&Src> {
        self.hir2src.get(idx)
    }
}

impl<Src, Hir> Default for SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone + Debug,
{
    fn default() -> Self {
        SourceMap { src2hir: FxHashMap::default(), hir2src: ArenaMap::default() }
    }
}

impl<Src, Hir> Index<Idx<Hir>> for SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone + Debug,
{
    type Output = Src;

    fn index(&self, index: Idx<Hir>) -> &Self::Output {
        &self.hir2src[index]
    }
}

impl<Src, Hir> Index<&Src> for SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone + Debug,
{
    type Output = Idx<Hir>;

    fn index(&self, index: &Src) -> &Self::Output {
        self.src2hir
            .get(index)
            .expect(&format!("Src {index:?} not found in SourceMap {:?}", self.hir2src))
    }
}

#[macro_export]
macro_rules! impl_source_map_idx {
    ($datas:ident for $($fld:ident[$src:ty, $hir:ty]),+ $(,)? ) => {
        $(
            impl Index<Idx<$hir>> for $datas {
                type Output = $src;
                fn index(&self, index: Idx<$hir>) -> &Self::Output {
                    &self.$fld[index]
                }
            }

            impl Index<&$src> for $datas {
                type Output = Idx<$hir>;
                fn index(&self, index: &$src) -> &Self::Output {
                    &self.$fld[index]
                }
            }
        )+
    };
}
