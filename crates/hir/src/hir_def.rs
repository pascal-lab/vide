pub mod bit;
pub mod block;
pub mod control;
pub mod data;
pub mod expr;
pub mod generate;
pub mod interface;
pub mod literal;
pub mod lower;
pub mod module;
pub mod pack_or_gen_item;
pub mod stmt;
pub mod tf;

use la_arena::{Arena, ArenaMap, Idx, IdxRange};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::{hash::Hash, ops::Index};
use syntax::ast::{self, ptr, AstNode};
use triomphe::Arc;
use utils::try_;

#[macro_export]
macro_rules! impl_index {
    ($datas:ident for $($tpy:ident, $fld:ident),+ $(,)? ) => {
        $(
            impl Index<Idx<$tpy>> for $datas {
                type Output = $tpy;
                fn index(&self, index: Idx<$tpy>) -> &Self::Output {
                    &self.$fld[index]
                }
            }

            impl Index<IdxRange<$tpy>> for $datas {
                type Output = [$tpy];
                fn index(& self, range: IdxRange<$tpy>) -> &Self::Output {
                    &self.$fld[range]
                }
            }
        )+
    };
}

pub(crate) use impl_index;

#[macro_export]
macro_rules! try_match {
    ($child:expr, $target:pat => $body:expr $(,)?) => {
        if let Some($target) = $child {
            $body
        }
    };

    (_ => $body:expr $(,)?) => { $body };

    ($child:expr, $target:pat => $body:expr, $($rest:tt)*) => {
        if let Some($target) = $child {
            $body
        } else {
            try_match!($($rest)*)
        }
    };
}

pub(crate) use try_match;

use crate::{container::InFile, db::HirDb, file::HirFileId};

pub type Ident = SmolStr;

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct HirFile {
    pub items: SmallVec<[FileItem; 1]>,
    pub data: FileData,
}

// TODO: DataDecl, InterfaceDecl
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum FileItem {
    Module(Idx<ModuleInfo>),
    // DataDecl(Idx<DataDecl>),
    // InterfaceDecl(Idx<InterfaceDecl>),
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub struct FileData {
    pub modules: Arena<ModuleInfo>,
    // pub data_decls: Arena<DataDecl>,
    // pub interface_decls: Arena<InterfaceDecl>,
}

impl FileData {
    pub fn shrink_to_fit(&mut self) {
        self.modules.shrink_to_fit();
        // self.data_decls.shrink_to_fit();
        // self.interface_decls.shrink_to_fit();
    }
}

impl_index! {FileData for ModuleInfo, modules}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ModuleInfo {
    pub ident: Ident,
}

pub type LocalModuleSrc = ptr::ModuleDeclarationPtr;
pub type ModuleSrc = InFile<LocalModuleSrc>;

pub type LocalModuleId = Idx<ModuleInfo>;
pub type ModuleId = InFile<LocalModuleId>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone,
{
    pub src2hir: FxHashMap<Src, Idx<Hir>>,
    pub hir2src: ArenaMap<Idx<Hir>, Src>,
}

impl<Src, Hir> SourceMap<Src, Hir>
where
    Src: PartialEq + Eq + Hash + Clone,
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
    Src: PartialEq + Eq + Hash + Clone,
{
    fn default() -> Self {
        SourceMap { src2hir: FxHashMap::default(), hir2src: ArenaMap::default() }
    }
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct FileSourceMap {
    pub modules: SourceMap<ModuleSrc, ModuleInfo>,
}

pub(crate) fn hir_file_with_source_map_query(
    db: &dyn HirDb,
    file_id: HirFileId,
) -> (Arc<HirFile>, Arc<FileSourceMap>) {
    let mut hir_file = HirFile::default();
    let mut source_map = FileSourceMap::default();
    db.hir_syntax_tree(file_id).and_then(|tree| {
        let root = ast::SourceFile::cast(tree.root_node())?;
        let file_text = db.hir_file_text(file_id);
        // FIXME: utf8_text panics if the identifier is not utf8

        for description in root.descriptions() {
            if let Some(module) = description.module_declaration() {
                try_! {
                    let ptr = module.to_ptr();
                    let ident = try_match! {
                        module.module_ansi_header(), header => {
                            header.identifier()?.to_text(&file_text)?.into()
                        },
                        module.module_nonansi_header(), header => {
                            header.identifier()?.to_text(&file_text)?.into()
                        },
                        _ => return None,
                    };
                    let module_id = hir_file.data.modules.alloc(ModuleInfo { ident });
                    hir_file.items.push(FileItem::Module(module_id));

                    let module_source = InFile::new(file_id, ptr);

                    source_map.modules.insert(module_source, module_id);
                };
            }
        }
        Some(())
    });
    hir_file.data.shrink_to_fit();
    (Arc::new(hir_file), Arc::new(source_map))
}
