use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    body::{Body, BodySourceMap},
    checker::CheckerId,
    covergroup::CovergroupId,
    db::HirDefDb,
    module::{ModuleId, ModuleKind, clocking::ClockingBlockId, generate::GenerateBlockId},
    owner::{OwnerId, OwnerKind},
    subroutine::LocalSubroutineId,
    symbol::ScopeKind,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InScope<T> {
    pub value: T,
    pub scope_id: OwnerId,
}

impl<T> InScope<T> {
    pub fn new(scope_id: OwnerId, value: T) -> Self {
        Self { value, scope_id }
    }

    pub fn with_value<U>(&self, value: U) -> InScope<U> {
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

    pub fn parent_owner(&self, db: &dyn HirDefDb) -> OwnerId {
        self.cont_id.owner(db)
    }
}

impl FileOrModule {
    pub fn file_id(self) -> HirFileId {
        match self {
            FileOrModule::File(file_id) => file_id,
            FileOrModule::Module(module_id) => module_id.file_id,
        }
    }

    pub fn owner(self, db: &dyn HirDefDb) -> OwnerId {
        match self {
            FileOrModule::File(file_id) => {
                db.owner_table(file_id).file_owner().expect("file owner")
            }
            FileOrModule::Module(module_id) => module_id.owner(db).expect("module owner"),
        }
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

    pub fn parent_owner(&self, db: &dyn HirDefDb) -> OwnerId {
        self.cont_id.owner(db)
    }

    pub fn file_id(self, db: &dyn HirDefDb) -> HirFileId {
        match self.cont_id {
            SubroutineParent::File(file_id) => file_id,
            SubroutineParent::Module(module_id) => module_id.file_id,
            SubroutineParent::GenerateBlock(generate_block_id) => generate_block_id.file_id(db),
        }
    }
}

impl SubroutineParent {
    pub fn owner(&self, db: &dyn HirDefDb) -> OwnerId {
        match self {
            SubroutineParent::File(file_id) => {
                db.owner_table(*file_id).file_owner().expect("file owner")
            }
            SubroutineParent::Module(module_id) => module_id.owner(db).expect("module owner"),
            SubroutineParent::GenerateBlock(block) => {
                block.clone().owner(db).expect("generate owner")
            }
        }
    }
}

/// An owner-local arena index. `OwnerId` selects the only store; `value` is
/// meaningful only inside that store.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct InContainer<T> {
    pub value: T,
    pub cont_id: OwnerId,
}

impl<T> InContainer<T> {
    pub fn new(cont_id: OwnerId, value: T) -> InContainer<T> {
        InContainer { value, cont_id }
    }

    pub fn with_value<U>(&self, value: U) -> InContainer<U> {
        InContainer::<U>::new(self.cont_id, value)
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

            // Typed wrappers retain their concrete container projection; they
            // are not implicitly converted into owner-local arena references.
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

impl OwnerId {
    pub fn as_checker(self, db: &dyn HirDefDb) -> Option<InFileOrModule<CheckerId>> {
        (self.kind(db) == OwnerKind::Checker).then_some(())?;
        let parent = self.parent(db)?;
        let cont_id = match parent.kind(db) {
            OwnerKind::File => FileOrModule::File(parent.file(db)),
            OwnerKind::Module => FileOrModule::Module(ModuleId::from_owner(db, parent)?),
            _ => return None,
        };
        let value = match cont_id {
            FileOrModule::File(file_id) => db
                .body_with_source_map(db.owner_table(file_id).file_owner().expect("file owner"))
                .source_map()
                .checker_srcs
                .src_to_hir(self.ast_id(db))?,
            FileOrModule::Module(module_id) => db
                .body_with_source_map(module_id.owner(db).expect("module owner"))
                .source_map()
                .checker_srcs
                .src_to_hir(self.ast_id(db))?,
        };
        Some(InFileOrModule::new(cont_id, value))
    }

    pub fn as_covergroup(self, db: &dyn HirDefDb) -> Option<InFileOrModule<CovergroupId>> {
        (self.kind(db) == OwnerKind::Covergroup).then_some(())?;
        let parent = self.parent(db)?;
        let cont_id = match parent.kind(db) {
            OwnerKind::File => FileOrModule::File(parent.file(db)),
            OwnerKind::Module => FileOrModule::Module(ModuleId::from_owner(db, parent)?),
            _ => return None,
        };
        let value = match cont_id {
            FileOrModule::File(file_id) => db
                .body_with_source_map(db.owner_table(file_id).file_owner().expect("file owner"))
                .source_map()
                .covergroup_srcs
                .src_to_hir(self.ast_id(db))?,
            FileOrModule::Module(module_id) => db
                .body_with_source_map(module_id.owner(db).expect("module owner"))
                .source_map()
                .covergroup_srcs
                .src_to_hir(self.ast_id(db))?,
        };
        Some(InFileOrModule::new(cont_id, value))
    }

    pub fn as_clocking_block(self, db: &dyn HirDefDb) -> Option<InModule<ClockingBlockId>> {
        (self.kind(db) == OwnerKind::ClockingBlock).then_some(())?;
        let module = ModuleId::from_owner(db, self.parent(db)?)?;
        let value = db
            .body_with_source_map(module.owner(db).expect("module owner"))
            .source_map()
            .clocking_block_srcs
            .src_to_hir(self.ast_id(db))?;
        Some(InModule::new(module, value))
    }

    pub fn scope_kind(self, db: &dyn HirDefDb) -> ScopeKind {
        match self.kind(db) {
            OwnerKind::File => ScopeKind::File,
            OwnerKind::Module => ModuleId::from_owner(db, self)
                .map(|module| {
                    match db
                        .body(db.owner_table(module.file_id).file_owner().expect("file owner"))
                        .get(module.value)
                        .kind
                    {
                        ModuleKind::Module => ScopeKind::Module,
                        ModuleKind::Interface => ScopeKind::Interface,
                        ModuleKind::Program => ScopeKind::Program,
                        ModuleKind::Package => ScopeKind::Package,
                    }
                })
                .unwrap_or(ScopeKind::Module),
            OwnerKind::GenerateBlock => ScopeKind::GenerateBlock,
            OwnerKind::ProceduralBlock => ScopeKind::ProceduralBlock,
            OwnerKind::Block => ScopeKind::Block,
            OwnerKind::Subroutine => ScopeKind::Subroutine,
            OwnerKind::Checker => ScopeKind::Checker,
            OwnerKind::Covergroup => ScopeKind::Covergroup,
            OwnerKind::ClockingBlock => ScopeKind::ClockingBlock,
        }
    }
}

/// Access to the canonical owner-local HIR store and source identities.
impl OwnerId {
    pub fn data(self, db: &dyn HirDefDb) -> Arc<Body> {
        db.body_with_source_map(self).data()
    }

    pub fn source_map(self, db: &dyn HirDefDb) -> Arc<BodySourceMap> {
        db.body_with_source_map(self).source_map_arc()
    }
}

impl GenerateBlockId {
    pub fn file_id(&self, _db: &dyn HirDefDb) -> HirFileId {
        self.loc().src.file_id
    }
}

/// An explicit lexical scope chain, ordered from the innermost scope outward.
///
/// Keeping the order in a value object prevents callers from rebuilding the
/// parent walk independently and accidentally changing shadowing precedence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeChain {
    ids: SmallVec<[OwnerId; 4]>,
}

impl ScopeChain {
    pub fn from_inner(db: &dyn HirDefDb, owner: OwnerId) -> Self {
        Self { ids: ScopeParent::start_from(db, owner).collect() }
    }

    pub fn ids(&self) -> &[OwnerId] {
        &self.ids
    }

    pub fn iter(&self) -> impl Iterator<Item = &OwnerId> {
        self.ids.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// Parents of a semantic owner.
pub struct ScopeParent<'db> {
    db: &'db dyn HirDefDb,
    owner: Option<OwnerId>,
}

impl<'db> ScopeParent<'db> {
    pub fn start_from(db: &'db dyn HirDefDb, owner: OwnerId) -> ScopeParent<'db> {
        ScopeParent { db, owner: Some(owner) }
    }
}

impl Iterator for ScopeParent<'_> {
    type Item = OwnerId;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.owner;
        self.owner = self.owner?.parent(self.db);
        next
    }
}
