use proc_macro_utils::impl_container;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::{define_enum_deriving_from, get::GetRef};
use vfs::FileId;

use crate::{
    base_db::intern::Lookup,
    db::{HirDb, InternDb},
    file::HirFileId,
    hir_def::{
        aggregate::{StructDef, StructId, StructSrc},
        block::{Block, BlockId, BlockInfo, BlockSourceMap, BlockSrc, LocalBlockId},
        checker::CheckerId,
        covergroup::CovergroupId,
        declaration::{Declaration, DeclarationId, DeclarationSrc},
        expr::{
            Expr, ExprId, ExprSrc,
            declarator::{DeclId, Declarator, DeclaratorSrc},
            timing_control::{EventExpr, EventExprId, EventExprSrc},
        },
        file::{FileSourceMap, HirFile},
        module::{
            Module, ModuleId, ModuleKind, ModuleSourceMap,
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
        Checker(InFileOrModule<CheckerId>),
        Covergroup(InFileOrModule<CovergroupId>),
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
    pub enum ArenaOwnerId {
        File(HirFileId),
        Module(ModuleId),
        GenerateBlock(GenerateBlockId),
        Block(BlockId),
        Subroutine(SubroutineScope),
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct InScope<T> {
    pub value: T,
    pub scope_id: ScopeId,
}

impl<T> InScope<T> {
    pub fn new(scope_id: ScopeId, value: T) -> Self {
        Self { value, scope_id }
    }

    pub fn with_value<U>(self, value: U) -> InScope<U> {
        InScope::new(self.scope_id, value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InScope<U> {
        InScope::new(self.scope_id, f(self.value))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct InFileOrModule<T> {
    pub value: T,
    pub cont_id: FileOrModule,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub enum FileOrModule {
    File(HirFileId),
    Module(ModuleId),
}

impl<T> InFileOrModule<T> {
    pub fn new(cont_id: FileOrModule, value: T) -> Self {
        Self { value, cont_id }
    }

    pub fn parent_scope(&self) -> ScopeId {
        self.cont_id.into()
    }
}

impl FileOrModule {
    pub fn file_id(self) -> FileId {
        match self {
            FileOrModule::File(file_id) => file_id.file_id(),
            FileOrModule::Module(module_id) => module_id.file_id(),
        }
    }
}

impl From<FileOrModule> for ArenaOwnerId {
    fn from(cont_id: FileOrModule) -> Self {
        match cont_id {
            FileOrModule::File(file_id) => file_id.into(),
            FileOrModule::Module(module_id) => module_id.into(),
        }
    }
}

impl From<FileOrModule> for ScopeId {
    fn from(cont_id: FileOrModule) -> Self {
        ArenaOwnerId::from(cont_id).into()
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

    pub fn file_id(self, db: &dyn InternDb) -> FileId {
        match self.cont_id {
            SubroutineParent::File(file_id) => file_id.file_id(),
            SubroutineParent::Module(module_id) => module_id.file_id(),
            SubroutineParent::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
        }
    }
}

impl From<SubroutineParent> for ArenaOwnerId {
    fn from(cont_id: SubroutineParent) -> Self {
        match cont_id {
            SubroutineParent::File(file_id) => file_id.into(),
            SubroutineParent::Module(module_id) => module_id.into(),
            SubroutineParent::GenerateBlock(generate_block_id) => generate_block_id.into(),
        }
    }
}

impl From<SubroutineParent> for ScopeId {
    fn from(cont_id: SubroutineParent) -> Self {
        ArenaOwnerId::from(cont_id).into()
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub cont_id: ArenaOwnerId,
}

impl<T> InContainer<T> {
    pub fn new(cont_id: ArenaOwnerId, value: T) -> InContainer<T> {
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
    pub subroutine: SubroutineScope,
}

impl<T> InSubroutine<T> {
    pub fn new(subroutine: SubroutineScope, value: T) -> Self {
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

impl From<ArenaOwnerId> for ScopeId {
    fn from(owner_id: ArenaOwnerId) -> Self {
        match owner_id {
            ArenaOwnerId::File(file_id) => file_id.into(),
            ArenaOwnerId::Module(module_id) => module_id.into(),
            ArenaOwnerId::GenerateBlock(generate_block_id) => generate_block_id.into(),
            ArenaOwnerId::Block(block_id) => block_id.into(),
            ArenaOwnerId::Subroutine(subroutine) => subroutine.into(),
        }
    }
}

impl ScopeId {
    pub fn kind(self, db: &dyn HirDb) -> ScopeKind {
        match self {
            ScopeId::File(_) => ScopeKind::File,
            ScopeId::Module(module_id) => {
                match db.hir_file(module_id.file_id).get(module_id.value).kind {
                    ModuleKind::Module => ScopeKind::Module,
                    ModuleKind::Interface => ScopeKind::Interface,
                    ModuleKind::Program => ScopeKind::Program,
                    ModuleKind::Package => ScopeKind::Package,
                }
            }
            ScopeId::GenerateBlock(_) => ScopeKind::GenerateBlock,
            ScopeId::Block(_) => ScopeKind::Block,
            ScopeId::Subroutine(_) => ScopeKind::Subroutine,
            ScopeId::ClockingBlock(_) => ScopeKind::ClockingBlock,
            ScopeId::Checker(_) => ScopeKind::Checker,
            ScopeId::Covergroup(_) => ScopeKind::Covergroup,
        }
    }

    pub fn name(self, db: &dyn HirDb) -> Option<SmolStr> {
        match self {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => db.module(module_id).name.clone(),
            ScopeId::GenerateBlock(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            ScopeId::Block(block_id) => db.block(block_id).name.clone(),
            ScopeId::Subroutine(subroutine) => db.subroutine(subroutine).name.clone(),
            ScopeId::ClockingBlock(clocking_block) => {
                db.module(clocking_block.module_id).get(clocking_block.value).name.clone()
            }
            ScopeId::Checker(checker) => match checker.cont_id {
                FileOrModule::File(file_id) => db.hir_file(file_id).get(checker.value).name.clone(),
                FileOrModule::Module(module_id) => {
                    db.module(module_id).get(checker.value).name.clone()
                }
            },
            ScopeId::Covergroup(covergroup) => match covergroup.cont_id {
                FileOrModule::File(file_id) => {
                    db.hir_file(file_id).get(covergroup.value).name.clone()
                }
                FileOrModule::Module(module_id) => {
                    db.module(module_id).get(covergroup.value).name.clone()
                }
            },
        }
    }

    pub fn arena_owner(self) -> Option<ArenaOwnerId> {
        match self {
            ScopeId::File(file_id) => Some(file_id.into()),
            ScopeId::Module(module_id) => Some(module_id.into()),
            ScopeId::GenerateBlock(generate_block_id) => Some(generate_block_id.into()),
            ScopeId::Block(block_id) => Some(block_id.into()),
            ScopeId::Subroutine(subroutine) => Some(subroutine.into()),
            ScopeId::ClockingBlock(_) | ScopeId::Checker(_) | ScopeId::Covergroup(_) => None,
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
            ScopeId::Checker(checker) => checker.cont_id.file_id(),
            ScopeId::Covergroup(covergroup) => covergroup.cont_id.file_id(),
        }
    }
}

/// Access to generic HIR arenas.
///
/// Name-resolution-only scopes cannot access arena data:
///
/// ```compile_fail
/// use hir::{container::ScopeId, db::HirDb};
///
/// fn data_for_any_scope(scope: ScopeId, db: &dyn HirDb) {
///     let _ = scope.data(db);
/// }
/// ```
impl ArenaOwnerId {
    pub fn file_id(self, db: &dyn InternDb) -> FileId {
        ScopeId::from(self).file_id(db)
    }

    pub fn data(self, db: &dyn HirDb) -> Container {
        match self {
            ArenaOwnerId::File(file_id) => file_id.to_container(db).into(),
            ArenaOwnerId::Module(module_id) => module_id.to_container(db).into(),
            ArenaOwnerId::GenerateBlock(generate_block_id) => {
                generate_block_id.to_container(db).into()
            }
            ArenaOwnerId::Block(block_id) => block_id.to_container(db).into(),
            ArenaOwnerId::Subroutine(subroutine) => db.subroutine(subroutine).into(),
        }
    }

    pub fn source_map(self, db: &dyn HirDb) -> ContainerSrcMap {
        match self {
            ArenaOwnerId::File(file_id) => file_id.to_container_src_map(db).into(),
            ArenaOwnerId::Module(module_id) => module_id.to_container_src_map(db).into(),
            ArenaOwnerId::GenerateBlock(generate_block_id) => {
                generate_block_id.to_container_src_map(db).into()
            }
            ArenaOwnerId::Block(block_id) => block_id.to_container_src_map(db).into(),
            ArenaOwnerId::Subroutine(subroutine) => {
                db.subroutine_with_source_map(subroutine).1.into()
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
                Some(generate_block_id.lookup(self.db).cont_id.into())
            }
            ScopeId::Block(block_id) => Some(block_id.lookup(self.db).cont_id.into()),
            ScopeId::Subroutine(subroutine) => Some(subroutine.parent_scope()),
            ScopeId::ClockingBlock(clocking_block) => Some(clocking_block.module_id.into()),
            ScopeId::Checker(checker) => Some(checker.parent_scope()),
            ScopeId::Covergroup(covergroup) => Some(covergroup.parent_scope()),
        };
        next
    }
}
