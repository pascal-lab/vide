use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use crate::{
    aggregate::{StructDef, StructId, StructSrc},
    block::{Block, BlockId, BlockInfo, BlockSourceMap, BlockSrc, LocalBlockId},
    checker::CheckerId,
    covergroup::CovergroupId,
    db::HirDefDb,
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
    owner::OwnerId,
    region_tree::RegionTree,
    stmt::{Stmt, StmtId, StmtSrc},
    subroutine::{LocalSubroutineId, Subroutine, SubroutineBody, SubroutineBodySourceMap},
    symbol::ScopeKind,
    typedef::{Typedef, TypedefId, TypedefSrc},
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
    pub enum ScopeId {
        File(HirFileId),
        Module(ModuleId),
        GenerateBlock(GenerateBlockId),
        Block(BlockId),
        Subroutine(SubroutineScope),
        Owner(OwnerId),
        ClockingBlock(InModule<ClockingBlockId>),
        Checker(InFileOrModule<CheckerId>),
        Covergroup(InFileOrModule<CovergroupId>),
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
    pub enum ArenaOwnerId {
        File(HirFileId),
        Module(ModuleId),
        GenerateBlock(GenerateBlockId),
        Block(BlockId),
        Subroutine(SubroutineScope),
        Owner(OwnerId),
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InScope<T> {
    pub value: T,
    pub scope_id: ScopeId,
}

impl<T> InScope<T> {
    pub fn new(scope_id: ScopeId, value: T) -> Self {
        Self { value, scope_id }
    }

    pub fn with_value<U>(&self, value: U) -> InScope<U> {
        InScope::new(self.scope_id.clone(), value)
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
    pub fn file_id(self) -> HirFileId {
        match self {
            FileOrModule::File(file_id) => file_id,
            FileOrModule::Module(module_id) => module_id.file_id,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct SubroutineScope {
    pub cont_id: SubroutineParent,
    pub value: LocalSubroutineId,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
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
        InContainer::new(self.cont_id.into(), self.value)
    }

    pub fn file_id(self, db: &dyn HirDefDb) -> HirFileId {
        match self.cont_id {
            SubroutineParent::File(file_id) => file_id,
            SubroutineParent::Module(module_id) => module_id.file_id,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub cont_id: ArenaOwnerId,
}

impl<T> InContainer<T> {
    pub fn new(cont_id: ArenaOwnerId, value: T) -> InContainer<T> {
        InContainer { value, cont_id }
    }

    pub fn with_value<U>(&self, value: U) -> InContainer<U> {
        InContainer::<U>::new(self.cont_id.clone(), value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InContainer<U> {
        InContainer::new(self.cont_id, f(self.value))
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
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
            #[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
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
}
impl<T: Copy> Copy for InFile<T> {}
impl<T: Copy> Copy for InModule<T> {}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InBlock<T> {
    pub value: T,
    pub block_id: BlockId,
}

impl<T> InBlock<T> {
    pub fn new(block_id: BlockId, value: T) -> Self {
        Self { value, block_id }
    }

    pub fn with_value<U>(self, value: U) -> InBlock<U> {
        InBlock::new(self.block_id, value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> InBlock<U> {
        InBlock::new(self.block_id, f(self.value))
    }
}

impl<T> From<InBlock<T>> for InContainer<T> {
    fn from(item: InBlock<T>) -> InContainer<T> {
        InContainer::new(item.block_id.into(), item.value)
    }
}

impl From<ArenaOwnerId> for ScopeId {
    fn from(owner_id: ArenaOwnerId) -> Self {
        match owner_id {
            ArenaOwnerId::File(file_id) => file_id.into(),
            ArenaOwnerId::Module(module_id) => module_id.into(),
            ArenaOwnerId::GenerateBlock(generate_block_id) => generate_block_id.into(),
            ArenaOwnerId::Block(block_id) => block_id.into(),
            ArenaOwnerId::Subroutine(subroutine) => subroutine.into(),
            ArenaOwnerId::Owner(owner) => ScopeId::Owner(owner),
        }
    }
}

impl ScopeId {
    pub fn kind(self, db: &dyn HirDefDb) -> ScopeKind {
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
            ScopeId::Subroutine(_) | ScopeId::Owner(_) => ScopeKind::Subroutine,
            ScopeId::ClockingBlock(_) => ScopeKind::ClockingBlock,
            ScopeId::Checker(_) => ScopeKind::Checker,
            ScopeId::Covergroup(_) => ScopeKind::Covergroup,
        }
    }

    pub fn name(self, db: &dyn HirDefDb) -> Option<SmolStr> {
        match self {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => db.module(module_id).name.clone(),
            ScopeId::GenerateBlock(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            ScopeId::Block(block_id) => db.block(block_id).name.clone(),
            ScopeId::Subroutine(subroutine) => db.subroutine(subroutine).name.clone(),
            ScopeId::Owner(owner) => db
                .owner_table(owner.file(db))
                .owners()
                .iter()
                .find(|data| data.id == owner)
                .map(|data| data.name.clone()),
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

    pub fn arena_owner(&self) -> Option<ArenaOwnerId> {
        match self {
            ScopeId::File(file_id) => Some((*file_id).into()),
            ScopeId::Module(module_id) => Some((*module_id).into()),
            ScopeId::GenerateBlock(generate_block_id) => Some(generate_block_id.clone().into()),
            ScopeId::Block(block_id) => Some(block_id.clone().into()),
            ScopeId::Subroutine(subroutine) => Some(subroutine.clone().into()),
            ScopeId::Owner(owner) => Some(ArenaOwnerId::Owner(*owner)),
            ScopeId::ClockingBlock(_) | ScopeId::Checker(_) | ScopeId::Covergroup(_) => None,
        }
    }

    pub fn file_id(&self, db: &dyn HirDefDb) -> HirFileId {
        match self {
            ScopeId::File(file_id) => *file_id,
            ScopeId::Module(module_id) => module_id.file_id,
            ScopeId::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
            ScopeId::Block(block_id) => block_id.file_id(db),
            ScopeId::Subroutine(subroutine) => subroutine.clone().file_id(db),
            ScopeId::Owner(owner) => owner.file(db),
            ScopeId::ClockingBlock(clocking_block) => clocking_block.module_id.file_id,
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
/// use hir_def::{container::ScopeId, db::HirDefDb};
///
/// fn data_for_any_scope(scope: ScopeId, db: &dyn HirDefDb) {
///     let _ = scope.data(db);
/// }
/// ```
impl ArenaOwnerId {
    pub fn file_id(&self, db: &dyn HirDefDb) -> HirFileId {
        ScopeId::from(self.clone()).file_id(db)
    }

    pub fn data(&self, db: &dyn HirDefDb) -> Container {
        match self {
            ArenaOwnerId::File(file_id) => Container::HirFile(db.hir_file(*file_id)),
            ArenaOwnerId::Module(module_id) => Container::Module(module_id.to_container(db)),
            ArenaOwnerId::GenerateBlock(generate_block_id) => {
                Container::GenerateBlock(generate_block_id.to_container(db))
            }
            ArenaOwnerId::Block(block_id) => Container::Block(block_id.to_container(db)),
            ArenaOwnerId::Subroutine(subroutine) => {
                Container::Subroutine(db.subroutine(subroutine.clone()))
            }
            ArenaOwnerId::Owner(owner) => {
                Container::SubroutineBody(db.subroutine_body_with_source_map(*owner).data())
            }
        }
    }

    pub fn source_map(&self, db: &dyn HirDefDb) -> ContainerSrcMap {
        match self {
            ArenaOwnerId::File(file_id) => {
                ContainerSrcMap::File(db.hir_file_with_source_map(*file_id).source_map_arc())
            }
            ArenaOwnerId::Module(module_id) => {
                ContainerSrcMap::Module(module_id.to_container_src_map(db))
            }
            ArenaOwnerId::GenerateBlock(generate_block_id) => {
                ContainerSrcMap::GenerateBlock(generate_block_id.to_container_src_map(db))
            }
            ArenaOwnerId::Block(block_id) => {
                ContainerSrcMap::Block(block_id.to_container_src_map(db))
            }
            ArenaOwnerId::Subroutine(subroutine) => ContainerSrcMap::SubroutineBody(
                db.subroutine_body_with_source_map(
                    subroutine.clone().owner(db).expect("subroutine must map to an owner"),
                )
                .source_map_arc(),
            ),
            ArenaOwnerId::Owner(owner) => ContainerSrcMap::SubroutineBody(
                db.subroutine_body_with_source_map(*owner).source_map_arc(),
            ),
        }
    }
}

impl ModuleId {
    #[inline]
    pub fn to_container(&self, db: &dyn HirDefDb) -> Arc<Module> {
        db.module(*self)
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDefDb) -> Arc<ModuleSourceMap> {
        db.module_with_source_map(*self).source_map_arc()
    }
}

impl BlockId {
    pub fn file_id(&self, _db: &dyn HirDefDb) -> HirFileId {
        self.loc().src.file_id
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDefDb) -> Arc<Block> {
        db.block(self.clone())
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDefDb) -> Arc<BlockSourceMap> {
        db.block_with_source_map(self.clone()).source_map_arc()
    }
}

impl GenerateBlockId {
    pub fn file_id(&self, _db: &dyn HirDefDb) -> HirFileId {
        self.loc().src.file_id
    }

    #[inline]
    pub fn to_container(&self, db: &dyn HirDefDb) -> Arc<GenerateBlock> {
        db.generate_block(self.clone())
    }

    #[inline]
    pub fn to_container_src_map(&self, db: &dyn HirDefDb) -> Arc<GenerateBlockSourceMap> {
        db.generate_block_with_source_map(self.clone()).source_map_arc()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Container {
    HirFile(Arc<HirFile>),
    Module(Arc<Module>),
    GenerateBlock(Arc<GenerateBlock>),
    Block(Arc<Block>),
    Subroutine(Arc<Subroutine>),
    SubroutineBody(Arc<SubroutineBody>),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ContainerSrcMap {
    File(Arc<FileSourceMap>),
    Module(Arc<ModuleSourceMap>),
    GenerateBlock(Arc<GenerateBlockSourceMap>),
    Block(Arc<BlockSourceMap>),
    Subroutine(Arc<SubroutineBodySourceMap>),
    SubroutineBody(Arc<SubroutineBodySourceMap>),
}

impl Container {
    pub fn name(&self) -> Option<&SmolStr> {
        match self {
            Container::HirFile(_) => None,
            Container::Module(module) => module.name.as_ref(),
            Container::GenerateBlock(generate_block) => generate_block.name.as_ref(),
            Container::Block(block) => block.name.as_ref(),
            Container::Subroutine(container) => container.name.as_ref(),
            Container::SubroutineBody(_) => None,
        }
    }

    pub fn declaration(&self, id: DeclarationId) -> &Declaration {
        match self {
            Container::HirFile(container) => &container.declarations[id],
            Container::Module(container) => &container.declarations[id],
            Container::GenerateBlock(container) => &container.declarations[id],
            Container::Block(container) => &container.declarations[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.declarations[id],
        }
    }

    pub fn typedef(&self, id: TypedefId) -> &Typedef {
        match self {
            Container::HirFile(container) => &container.typedefs[id],
            Container::Module(container) => &container.typedefs[id],
            Container::GenerateBlock(container) => &container.typedefs[id],
            Container::Block(container) => &container.typedefs[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.typedefs[id],
        }
    }

    pub fn struct_def(&self, id: StructId) -> &StructDef {
        match self {
            Container::HirFile(container) => &container.structs[id],
            Container::Module(container) => &container.structs[id],
            Container::GenerateBlock(container) => &container.structs[id],
            Container::Block(container) => &container.structs[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.structs[id],
        }
    }

    pub fn expr(&self, id: ExprId) -> &Expr {
        match self {
            Container::HirFile(container) => &container.exprs[id],
            Container::Module(container) => &container.exprs[id],
            Container::GenerateBlock(container) => &container.exprs[id],
            Container::Block(container) => &container.exprs[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.exprs[id],
        }
    }

    pub fn event_expr(&self, id: EventExprId) -> &EventExpr {
        match self {
            Container::HirFile(container) => &container.event_exprs[id],
            Container::Module(container) => &container.event_exprs[id],
            Container::GenerateBlock(container) => &container.event_exprs[id],
            Container::Block(container) => &container.event_exprs[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.event_exprs[id],
        }
    }

    pub fn declarator(&self, id: DeclId) -> &Declarator {
        match self {
            Container::HirFile(container) => &container.decls[id],
            Container::Module(container) => &container.decls[id],
            Container::GenerateBlock(container) => &container.decls[id],
            Container::Block(container) => &container.decls[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.decls[id],
        }
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        match self {
            Container::HirFile(container) => &container.stmts[id],
            Container::Module(container) => &container.stmts[id],
            Container::GenerateBlock(container) => &container.stmts[id],
            Container::Block(container) => &container.stmts[id],
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => &container.stmts[id],
        }
    }

    pub fn block_info(&self, id: LocalBlockId) -> &BlockInfo {
        match self {
            Container::HirFile(container) => utils::get::GetRef::get(&container.stmts, id),
            Container::Module(container) => utils::get::GetRef::get(&container.stmts, id),
            Container::GenerateBlock(container) => utils::get::GetRef::get(&container.stmts, id),
            Container::Block(container) => utils::get::GetRef::get(&container.stmts, id),
            Container::Subroutine(_) => unreachable!("subroutine skeleton has no body arenas"),
            Container::SubroutineBody(container) => utils::get::GetRef::get(&container.stmts, id),
        }
    }
}

impl ContainerSrcMap {
    pub fn region_tree(&self) -> &RegionTree {
        match self {
            ContainerSrcMap::File(container) => &container.region_tree,
            ContainerSrcMap::Module(container) => &container.region_tree,
            ContainerSrcMap::GenerateBlock(container) => &container.region_tree,
            ContainerSrcMap::Block(container) => &container.region_tree,
            ContainerSrcMap::Subroutine(container) => &container.region_tree,
            ContainerSrcMap::SubroutineBody(container) => &container.region_tree,
        }
    }

    pub fn declaration_from_source(&self, src: DeclarationSrc) -> Option<DeclarationId> {
        match self {
            ContainerSrcMap::File(container) => container.declaration_srcs.get(src),
            ContainerSrcMap::Module(container) => container.declaration_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.declaration_srcs.get(src),
            ContainerSrcMap::Block(container) => container.declaration_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.declaration_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.declaration_srcs.get(src),
        }
    }

    pub fn source_of_declaration(&self, id: DeclarationId) -> Option<DeclarationSrc> {
        match self {
            ContainerSrcMap::File(container) => container.declaration_srcs.get(id),
            ContainerSrcMap::Module(container) => container.declaration_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.declaration_srcs.get(id),
            ContainerSrcMap::Block(container) => container.declaration_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.declaration_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.declaration_srcs.get(id),
        }
    }

    pub fn typedef_from_source(&self, src: TypedefSrc) -> Option<TypedefId> {
        match self {
            ContainerSrcMap::File(container) => container.typedef_srcs.get(src),
            ContainerSrcMap::Module(container) => container.typedef_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.typedef_srcs.get(src),
            ContainerSrcMap::Block(container) => container.typedef_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.typedef_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.typedef_srcs.get(src),
        }
    }

    pub fn source_of_typedef(&self, id: TypedefId) -> Option<TypedefSrc> {
        match self {
            ContainerSrcMap::File(container) => container.typedef_srcs.get(id),
            ContainerSrcMap::Module(container) => container.typedef_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.typedef_srcs.get(id),
            ContainerSrcMap::Block(container) => container.typedef_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.typedef_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.typedef_srcs.get(id),
        }
    }

    pub fn struct_from_source(&self, src: StructSrc) -> Option<StructId> {
        match self {
            ContainerSrcMap::File(container) => container.struct_srcs.get(src),
            ContainerSrcMap::Module(container) => container.struct_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.struct_srcs.get(src),
            ContainerSrcMap::Block(container) => container.struct_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.struct_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.struct_srcs.get(src),
        }
    }

    pub fn source_of_struct(&self, id: StructId) -> Option<StructSrc> {
        match self {
            ContainerSrcMap::File(container) => container.struct_srcs.get(id),
            ContainerSrcMap::Module(container) => container.struct_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.struct_srcs.get(id),
            ContainerSrcMap::Block(container) => container.struct_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.struct_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.struct_srcs.get(id),
        }
    }

    pub fn expr_from_source(&self, src: ExprSrc) -> Option<ExprId> {
        match self {
            ContainerSrcMap::File(container) => container.expr_srcs.get(src),
            ContainerSrcMap::Module(container) => container.expr_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.expr_srcs.get(src),
            ContainerSrcMap::Block(container) => container.expr_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.expr_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.expr_srcs.get(src),
        }
    }

    pub fn source_of_expr(&self, id: ExprId) -> Option<ExprSrc> {
        match self {
            ContainerSrcMap::File(container) => container.expr_srcs.get(id),
            ContainerSrcMap::Module(container) => container.expr_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.expr_srcs.get(id),
            ContainerSrcMap::Block(container) => container.expr_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.expr_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.expr_srcs.get(id),
        }
    }

    pub fn event_expr_from_source(&self, src: EventExprSrc) -> Option<EventExprId> {
        match self {
            ContainerSrcMap::File(container) => container.event_expr_srcs.get(src),
            ContainerSrcMap::Module(container) => container.event_expr_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.event_expr_srcs.get(src),
            ContainerSrcMap::Block(container) => container.event_expr_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.event_expr_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.event_expr_srcs.get(src),
        }
    }

    pub fn source_of_event_expr(&self, id: EventExprId) -> Option<EventExprSrc> {
        match self {
            ContainerSrcMap::File(container) => container.event_expr_srcs.get(id),
            ContainerSrcMap::Module(container) => container.event_expr_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.event_expr_srcs.get(id),
            ContainerSrcMap::Block(container) => container.event_expr_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.event_expr_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.event_expr_srcs.get(id),
        }
    }

    pub fn declarator_from_source(&self, src: DeclaratorSrc) -> Option<DeclId> {
        match self {
            ContainerSrcMap::File(container) => container.decl_srcs.get(src),
            ContainerSrcMap::Module(container) => container.decl_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.decl_srcs.get(src),
            ContainerSrcMap::Block(container) => container.decl_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.decl_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.decl_srcs.get(src),
        }
    }

    pub fn source_of_declarator(&self, id: DeclId) -> Option<DeclaratorSrc> {
        match self {
            ContainerSrcMap::File(container) => container.decl_srcs.get(id),
            ContainerSrcMap::Module(container) => container.decl_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.decl_srcs.get(id),
            ContainerSrcMap::Block(container) => container.decl_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.decl_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.decl_srcs.get(id),
        }
    }

    pub fn stmt_from_source(&self, src: StmtSrc) -> Option<StmtId> {
        match self {
            ContainerSrcMap::File(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::Module(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::Block(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.stmt_srcs.get(src),
        }
    }

    pub fn source_of_stmt(&self, id: StmtId) -> Option<StmtSrc> {
        match self {
            ContainerSrcMap::File(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::Module(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::Block(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.stmt_srcs.get(id),
        }
    }

    pub fn block_from_source(&self, src: BlockSrc) -> Option<LocalBlockId> {
        match self {
            ContainerSrcMap::File(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::Module(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::GenerateBlock(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::Block(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::Subroutine(container) => container.stmt_srcs.get(src),
            ContainerSrcMap::SubroutineBody(container) => container.stmt_srcs.get(src),
        }
    }

    pub fn source_of_block(&self, id: LocalBlockId) -> Option<BlockSrc> {
        match self {
            ContainerSrcMap::File(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::Module(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::GenerateBlock(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::Block(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::Subroutine(container) => container.stmt_srcs.get(id),
            ContainerSrcMap::SubroutineBody(container) => container.stmt_srcs.get(id),
        }
    }
}

/// An explicit lexical scope chain, ordered from the innermost scope outward.
///
/// Keeping the order in a value object prevents callers from rebuilding the
/// parent walk independently and accidentally changing shadowing precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeChain {
    ids: SmallVec<[ScopeId; 4]>,
}

impl ScopeChain {
    pub fn from_inner(scope_id: ScopeId) -> Self {
        Self { ids: ScopeParent::start_from(scope_id).collect() }
    }

    pub fn ids(&self) -> &[ScopeId] {
        &self.ids
    }

    pub fn iter(&self) -> impl Iterator<Item = &ScopeId> {
        self.ids.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Parents of a scope.
pub struct ScopeParent {
    cont_id: Option<ScopeId>,
}

impl ScopeParent {
    pub fn start_from(cont_id: ScopeId) -> ScopeParent {
        ScopeParent { cont_id: Some(cont_id) }
    }
}

impl Iterator for ScopeParent {
    type Item = ScopeId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.cont_id.clone();
        self.cont_id = match self.cont_id.clone()? {
            ScopeId::File(_) => None,
            ScopeId::Module(module_id) => Some(module_id.file_id.into()),
            ScopeId::GenerateBlock(generate_block_id) => {
                Some(generate_block_id.loc().cont_id.clone().into())
            }
            ScopeId::Block(block_id) => Some(block_id.loc().cont_id.clone().into()),
            ScopeId::Subroutine(subroutine) => Some(subroutine.parent_scope()),
            ScopeId::Owner(_) => None,
            ScopeId::ClockingBlock(clocking_block) => Some(clocking_block.module_id.into()),
            ScopeId::Checker(checker) => Some(checker.parent_scope()),
            ScopeId::Covergroup(covergroup) => Some(covergroup.parent_scope()),
        };
        next
    }
}
