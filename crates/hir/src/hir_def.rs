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

use std::{fmt::Debug, hash::Hash, ops::Index};

use la_arena::{Arena, Idx, IdxRange};
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::ast::{self, ptr, AstNode};
use triomphe::Arc;
use utils::try_;

#[macro_export]
macro_rules! impl_arena_idx {
    ($datas:ident for $($fld:ident[$ty:ident]),+ $(,)? ) => {
        $(
            impl Index<Idx<$ty>> for $datas {
                type Output = $ty;
                fn index(&self, index: Idx<$ty>) -> &Self::Output {
                    &self.$fld[index]
                }
            }

            impl Index<IdxRange<$ty>> for $datas {
                type Output = [$ty];
                fn index(&self, range: IdxRange<$ty>) -> &Self::Output {
                    &self.$fld[range]
                }
            }
        )+
    };
}

use impl_arena_idx;

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

use crate::{
    container::InFile, db::HirDb, file::HirFileId, impl_source_map_idx, source_map::SourceMap,
};

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

impl_arena_idx! {FileData for modules[ModuleInfo]}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ModuleInfo {
    pub ident: Ident,
}

pub type LocalModuleSrc = ptr::ModuleDeclarationPtr;
pub type ModuleSrc = InFile<LocalModuleSrc>;

pub type LocalModuleId = Idx<ModuleInfo>;
pub type ModuleId = InFile<LocalModuleId>;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct FileSourceMap {
    pub modules: SourceMap<ModuleSrc, ModuleInfo>,
}

impl_source_map_idx! { FileSourceMap for
    modules[ModuleSrc, ModuleInfo]
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
