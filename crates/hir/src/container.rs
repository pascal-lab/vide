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
            clocking::ClockingBlockId,
            generate::{GenerateBlock, GenerateBlockId, GenerateBlockSourceMap},
        },
        stmt::{Stmt, StmtId, StmtSrc},
        subroutine::{LocalSubroutineId, Subroutine, SubroutineSourceMap},
        typedef::{Typedef, TypedefId, TypedefSrc},
    },
    region_tree::RegionTree,
    symbol::ScopeKind,
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
    pub enum ScopeId {
        File(HirFileId),
        Module(ModuleId),
        GenerateBlock(GenerateBlockId),
        Block(BlockId),
        Subroutine(SubroutineScope),
        ClockingBlock(InModule<ClockingBlockId>),
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct SubroutineScope {
    pub cont_id: SubroutineParent,
    pub value: LocalSubroutineId,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum SubroutineParent {
    File(HirFileId),
    Module(ModuleId),
    GenerateBlock(GenerateBlockId),
}

impl SubroutineScope {
    pub fn new(cont_id: SubroutineParent, value: LocalSubroutineId) -> Self {
        Self { cont_id, value }
    }

    pub fn parent_scope(self) -> ScopeId {
        self.cont_id.into()
    }

    pub fn as_in_container(self) -> InContainer<LocalSubroutineId> {
        InContainer::new(self.parent_scope(), self.value)
    }

    pub fn file_id(self, db: &dyn InternDb) -> FileId {
        match self.cont_id {
            SubroutineParent::File(file_id) => file_id.file_id(),
            SubroutineParent::Module(module_id) => module_id.file_id(),
            SubroutineParent::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
        }
    }
}

impl From<SubroutineParent> for ScopeId {
    fn from(cont_id: SubroutineParent) -> Self {
        match cont_id {
            SubroutineParent::File(file_id) => file_id.into(),
            SubroutineParent::Module(module_id) => module_id.into(),
            SubroutineParent::GenerateBlock(generate_block_id) => generate_block_id.into(),
        }
    }
}

impl TryFrom<ScopeId> for SubroutineParent {
    type Error = ();

    fn try_from(cont_id: ScopeId) -> Result<Self, Self::Error> {
        match cont_id {
            ScopeId::File(file_id) => Ok(Self::File(file_id)),
            ScopeId::Module(module_id) => Ok(Self::Module(module_id)),
            ScopeId::GenerateBlock(generate_block_id) => Ok(Self::GenerateBlock(generate_block_id)),
            ScopeId::Block(_) | ScopeId::Subroutine(_) | ScopeId::ClockingBlock(_) => Err(()),
        }
    }
}

impl From<InContainer<LocalSubroutineId>> for SubroutineScope {
    fn from(subroutine: InContainer<LocalSubroutineId>) -> Self {
        let parent = SubroutineParent::try_from(subroutine.cont_id)
            .expect("subroutines are lowered only in file, module, or generate-block scopes");
        Self::new(parent, subroutine.value)
    }
}

impl From<InContainer<LocalSubroutineId>> for ScopeId {
    fn from(subroutine: InContainer<LocalSubroutineId>) -> Self {
        ScopeId::Subroutine(subroutine.into())
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub cont_id: ScopeId,
}

impl<T> InContainer<T> {
    pub fn new(cont_id: ScopeId, value: T) -> InContainer<T> {
        InContainer { value, cont_id }
    }

    pub fn with_value<U>(self, value: U) -> InContainer<U> {
        InContainer::<U>::new(self.cont_id, value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InContainer<U> {
        InContainer::new(self.cont_id, f(self.value))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct InSubroutine<T> {
    pub value: T,
    pub subroutine: InContainer<LocalSubroutineId>,
}

impl<T> InSubroutine<T> {
    pub fn new(subroutine: InContainer<LocalSubroutineId>, value: T) -> Self {
        Self { value, subroutine }
    }

    pub fn with_value<U>(self, value: U) -> InSubroutine<U> {
        InSubroutine { value, subroutine: self.subroutine }
    }
}

impl<T> From<InSubroutine<T>> for InContainer<T> {
    fn from(item: InSubroutine<T>) -> InContainer<T> {
        InContainer::new(item.subroutine.into(), item.value)
    }
}

macro_rules! define_container_id {
    ($($name:ident[$id:ident : $ty:ty]),* $(,)?) => {
        $(
            #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
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

impl ScopeId {
    pub fn kind(self) -> ScopeKind {
        match self {
            ScopeId::File(_) => ScopeKind::File,
            ScopeId::Module(_) => ScopeKind::Module,
            ScopeId::GenerateBlock(_) => ScopeKind::GenerateBlock,
            ScopeId::Block(_) => ScopeKind::Block,
            ScopeId::Subroutine(_) => ScopeKind::Subroutine,
            ScopeId::ClockingBlock(_) => ScopeKind::ClockingBlock,
        }
    }

    pub fn file_id(self, db: &dyn InternDb) -> FileId {
        match self {
            ScopeId::File(file_id) => file_id.file_id(),
            ScopeId::Module(module_id) => module_id.file_id(),
            ScopeId::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
            ScopeId::Block(block_id) => block_id.file_id(db),
            ScopeId::Subroutine(subroutine) => subroutine.file_id(db),
            ScopeId::ClockingBlock(clocking_block) => clocking_block.module_id.file_id(),
        }
    }

    pub fn to_container(self, db: &dyn HirDb) -> Container {
        match self {
            ScopeId::File(file_id) => file_id.to_container(db).into(),
            ScopeId::Module(module_id) => module_id.to_container(db).into(),
            ScopeId::GenerateBlock(generate_block_id) => generate_block_id.to_container(db).into(),
            ScopeId::Block(block_id) => block_id.to_container(db).into(),
            ScopeId::Subroutine(subroutine) => db.subroutine(subroutine.as_in_container()).into(),
            ScopeId::ClockingBlock(_) => {
                panic!("clocking block scopes do not expose a generic HIR container")
            }
        }
    }

    pub fn to_container_src_map(self, db: &dyn HirDb) -> ContainerSrcMap {
        match self {
            ScopeId::File(file_id) => file_id.to_container_src_map(db).into(),
            ScopeId::Module(module_id) => module_id.to_container_src_map(db).into(),
            ScopeId::GenerateBlock(generate_block_id) => {
                generate_block_id.to_container_src_map(db).into()
            }
            ScopeId::Block(block_id) => block_id.to_container_src_map(db).into(),
            ScopeId::Subroutine(subroutine) => {
                db.subroutine_with_source_map(subroutine.as_in_container()).1.into()
            }
            ScopeId::ClockingBlock(_) => {
                panic!("clocking block scopes do not expose a generic source map")
            }
        }
    }
}

impl HirFileId {
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
pub struct ScopeParent<'db> {
    db: &'db dyn InternDb,
    cont_id: Option<ScopeId>,
}

impl ScopeParent<'_> {
    pub fn start_from(db: &dyn InternDb, cont_id: ScopeId) -> ScopeParent<'_> {
        ScopeParent { db, cont_id: Some(cont_id) }
    }
}

impl Iterator for ScopeParent<'_> {
    type Item = ScopeId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.cont_id;
        self.cont_id = match self.cont_id? {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => Some(module_id.file_id.into()),
            ScopeId::GenerateBlock(generate_block_id) => {
                Some(generate_block_id.lookup(self.db).cont_id)
            }
            ScopeId::Block(block_id) => Some(block_id.lookup(self.db).cont_id),
            ScopeId::Subroutine(subroutine) => Some(subroutine.parent_scope()),
            ScopeId::ClockingBlock(clocking_block) => Some(clocking_block.module_id.into()),
        };
        next
    }
}
