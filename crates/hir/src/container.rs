use proc_macro_utils::impl_container;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::define_enum_deriving_from;
use vfs::FileId;

use crate::{
    base_db::intern::Lookup,
    db::{HirDb, InternDb},
    file::HirFileId,
    hir_def::{
        aggregate::{StructDef, StructId, StructSrc},
        block::{Block, BlockId, BlockInfo, BlockSourceMap, BlockSrc, LocalBlockId},
        declaration::{Declaration, DeclarationId, DeclarationSrc},
        expr::{
            Expr, ExprId, ExprSrc,
            declarator::{DeclId, Declarator, DeclaratorSrc},
            timing_control::{EventExpr, EventExprId, EventExprSrc},
        },
        file::{FileSourceMap, HirFile},
        module::{
            Module, ModuleId, ModuleSourceMap,
            generate::{GenerateBlock, GenerateBlockId, GenerateBlockSourceMap},
        },
        stmt::{Stmt, StmtId, StmtSrc},
        subroutine::{Subroutine, SubroutineId, SubroutineSourceMap},
        typedef::{Typedef, TypedefId, TypedefSrc},
    },
    region_tree::RegionTree,
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum ContainerId {
        HirFileId(HirFileId),
        ModuleId(ModuleId),
        GenerateBlockId(GenerateBlockId),
        BlockId(BlockId),
        SubroutineId(SubroutineId),
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub cont_id: ContainerId,
}

impl<T> InContainer<T> {
    pub fn new(cont_id: ContainerId, value: T) -> InContainer<T> {
        InContainer { value, cont_id }
    }

    pub fn with_value<U>(self, value: U) -> InContainer<U> {
        InContainer::<U>::new(self.cont_id, value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InContainer<U> {
        InContainer::new(self.cont_id, f(self.value))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct InSubroutine<T> {
    pub value: T,
    pub subroutine: SubroutineId,
}

impl<T> InSubroutine<T> {
    pub fn new(subroutine: SubroutineId, value: T) -> Self {
        Self { value, subroutine }
    }

    pub fn with_value<U>(self, value: U) -> InSubroutine<U> {
        InSubroutine { value, subroutine: self.subroutine }
    }
}

impl<T> From<InSubroutine<T>> for InContainer<T> {
    fn from(item: InSubroutine<T>) -> InContainer<T> {
        InContainer::new(ContainerId::SubroutineId(item.subroutine), item.value)
    }
}

macro_rules! define_container_id {
    ($($name:ident[$id:ident : $ty:ty]),* $(,)?) => {
        $(
            #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
            pub struct $name<T> {
                pub value: T,
                pub $id: $ty,
            }

            impl<T> $name<T> {
                pub fn new($id: $ty, value: T) -> Self {
                    Self { value, $id }
                }

                pub fn with_value<U>(self, value: U) -> $name<U> {
                    $name::<U>::new(self.$id, value)
                }

                pub fn map<U>(self, f: impl FnOnce(T) -> U) -> $name<U> {
                    $name::new(self.$id, f(self.value))
                }
            }

            impl<T> From<$name<T>> for InContainer<T> {
                fn from(item: $name<T>) -> InContainer<T> {
                    InContainer::new(item.$id.into(), item.value)
                }
            }
        )*
    };
}

define_container_id! {
    InFile[file_id: HirFileId],
    InModule[module_id: ModuleId],
    InGenerateBlock[generate_block_id: GenerateBlockId],
    InBlock[block_id: BlockId],
}

impl ContainerId {
    pub fn file_id(self, db: &dyn InternDb) -> FileId {
        match self {
            ContainerId::HirFileId(file_id) => file_id.file_id(),
            ContainerId::ModuleId(module_id) => module_id.file_id(),
            ContainerId::GenerateBlockId(generate_block_id) => generate_block_id.file_id(db),
            ContainerId::BlockId(block_id) => block_id.file_id(db),
            ContainerId::SubroutineId(subroutine_id) => {
                subroutine_id.lookup(db).src.file_id.file_id()
            }
        }
    }

    pub fn to_container(self, db: &dyn HirDb) -> Container {
        match self {
            ContainerId::HirFileId(file_id) => file_id.to_container(db).into(),
            ContainerId::ModuleId(module_id) => module_id.to_container(db).into(),
            ContainerId::GenerateBlockId(generate_block_id) => {
                generate_block_id.to_container(db).into()
            }
            ContainerId::BlockId(block_id) => block_id.to_container(db).into(),
            ContainerId::SubroutineId(subroutine_id) => db.subroutine(subroutine_id).into(),
        }
    }

    pub fn to_container_src_map(self, db: &dyn HirDb) -> ContainerSrcMap {
        match self {
            ContainerId::HirFileId(file_id) => file_id.to_container_src_map(db).into(),
            ContainerId::ModuleId(module_id) => module_id.to_container_src_map(db).into(),
            ContainerId::GenerateBlockId(generate_block_id) => {
                generate_block_id.to_container_src_map(db).into()
            }
            ContainerId::BlockId(block_id) => block_id.to_container_src_map(db).into(),
            ContainerId::SubroutineId(subroutine_id) => {
                db.subroutine_with_source_map(subroutine_id).1.into()
            }
        }
    }
}

impl HirFileId {
    pub fn file_id(&self) -> FileId {
        self.0
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDb) -> Arc<HirFile> {
        db.hir_file(*self)
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDb) -> Arc<FileSourceMap> {
        db.hir_file_with_source_map(*self).1
    }
}

impl ModuleId {
    pub fn file_id(&self) -> FileId {
        self.file_id.file_id()
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDb) -> Arc<Module> {
        db.module(*self)
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDb) -> Arc<ModuleSourceMap> {
        db.module_with_source_map(*self).1
    }
}

impl BlockId {
    pub fn file_id(&self, db: &dyn InternDb) -> FileId {
        self.lookup(db).src.file_id.file_id()
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDb) -> Arc<Block> {
        db.block(*self)
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDb) -> Arc<BlockSourceMap> {
        db.block_with_source_map(*self).1
    }
}

impl GenerateBlockId {
    pub fn file_id(&self, db: &dyn InternDb) -> FileId {
        self.lookup(db).src.file_id.file_id()
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDb) -> Arc<GenerateBlock> {
        db.generate_block(*self)
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDb) -> Arc<GenerateBlockSourceMap> {
        db.generate_block_with_source_map(*self).1
    }
}

impl_container! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum {
        HirFile | FileSourceMap,
        Module | ModuleSourceMap,
        GenerateBlock | GenerateBlockSourceMap,
        Block | BlockSourceMap,
        Subroutine | SubroutineSourceMap,
    } => {
        Declaration[DeclarationId | DeclarationSrc],
        Typedef[TypedefId | TypedefSrc],
        StructDef[StructId | StructSrc],
        Expr[ExprId | ExprSrc],
        EventExpr[EventExprId | EventExprSrc],
        Declarator[DeclId | DeclaratorSrc],
        Stmt[StmtId | StmtSrc],
        BlockInfo[LocalBlockId | BlockSrc],
    }
}

impl Container {
    #[inline]
    pub fn name(&self) -> Option<&SmolStr> {
        match self {
            Container::HirFile(_) => None,
            Container::Module(module) => module.name.as_ref(),
            Container::GenerateBlock(generate_block) => generate_block.name.as_ref(),
            Container::Block(block) => block.name.as_ref(),
            Container::Subroutine(subroutine) => subroutine.name.as_ref(),
        }
    }
}

impl AsRef<Container> for Container {
    fn as_ref(&self) -> &Container {
        self
    }
}

impl ContainerSrcMap {
    #[inline]
    pub fn region_tree(&self) -> Option<&RegionTree> {
        match self {
            ContainerSrcMap::FileSourceMap(file) => Some(&file.region_tree),
            ContainerSrcMap::ModuleSourceMap(module) => Some(&module.region_tree),
            ContainerSrcMap::GenerateBlockSourceMap(generate_block) => {
                Some(&generate_block.region_tree)
            }
            ContainerSrcMap::BlockSourceMap(block) => Some(&block.region_tree),
            ContainerSrcMap::SubroutineSourceMap(subroutine) => Some(&subroutine.region_tree),
        }
    }
}

impl AsRef<ContainerSrcMap> for ContainerSrcMap {
    fn as_ref(&self) -> &ContainerSrcMap {
        self
    }
}

/// Parents of a scope.
pub struct ContainerParent<'db> {
    db: &'db dyn InternDb,
    cont_id: Option<ContainerId>,
}

impl ContainerParent<'_> {
    pub fn start_from(db: &dyn InternDb, cont_id: ContainerId) -> ContainerParent<'_> {
        ContainerParent { db, cont_id: Some(cont_id) }
    }
}

impl Iterator for ContainerParent<'_> {
    type Item = ContainerId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.cont_id;
        self.cont_id = match self.cont_id? {
            ContainerId::HirFileId(_) => None,
            ContainerId::ModuleId(module_id) => Some(module_id.file_id.into()),
            ContainerId::GenerateBlockId(generate_block_id) => {
                Some(generate_block_id.lookup(self.db).cont_id)
            }
            ContainerId::BlockId(block_id) => Some(block_id.lookup(self.db).cont_id),
            ContainerId::SubroutineId(subroutine_id) => {
                Some(subroutine_id.lookup(self.db).cont_id.into())
            }
        };
        next
    }
}
