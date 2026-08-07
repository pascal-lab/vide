use preproc_expand::file::HirFileId;
use smallvec::SmallVec;
use triomphe::Arc;

use crate::{
    body::{Body, BodySourceMap},
    checker::CheckerId,
    covergroup::CovergroupId,
    db::HirDefDb,
    module::{ModuleKind, clocking::ClockingBlockId},
    owner::{OwnerId, OwnerKind},
    symbol::ScopeKind,
};

/// An owner-local arena index. `OwnerId` selects the only store; `value` is
/// meaningful only inside that store.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash)]
pub struct OwnerRef<T> {
    pub value: T,
    pub cont_id: OwnerId,
}

impl<T> OwnerRef<T> {
    pub fn new(cont_id: OwnerId, value: T) -> OwnerRef<T> {
        OwnerRef { value, cont_id }
    }

    pub fn with_value<U>(&self, value: U) -> OwnerRef<U> {
        OwnerRef::<U>::new(self.cont_id, value)
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> OwnerRef<U> {
        OwnerRef::new(self.cont_id, f(self.value))
    }
}

impl<T: Copy> Copy for OwnerRef<T> {}

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
}
impl<T: Copy> Copy for InFile<T> {}

impl OwnerId {
    pub fn as_checker(self, db: &dyn HirDefDb) -> Option<OwnerRef<CheckerId>> {
        (self.kind(db) == OwnerKind::Checker)
            .then_some(OwnerRef::new(self, self.data(db).checkers.iter().next()?.0))
    }

    pub fn as_covergroup(self, db: &dyn HirDefDb) -> Option<OwnerRef<CovergroupId>> {
        (self.kind(db) == OwnerKind::Covergroup)
            .then_some(OwnerRef::new(self, self.data(db).covergroups.iter().next()?.0))
    }

    pub fn as_clocking_block(self, db: &dyn HirDefDb) -> Option<OwnerRef<ClockingBlockId>> {
        (self.kind(db) == OwnerKind::ClockingBlock)
            .then_some(OwnerRef::new(self, self.data(db).clocking_blocks.iter().next()?.0))
    }

    pub fn scope_kind(self, db: &dyn HirDefDb) -> ScopeKind {
        match self.kind(db) {
            OwnerKind::File => ScopeKind::File,
            OwnerKind::Module => match self.module_kind(db).expect("module owner must have a kind")
            {
                ModuleKind::Module => ScopeKind::Module,
                ModuleKind::Interface => ScopeKind::Interface,
                ModuleKind::Program => ScopeKind::Program,
                ModuleKind::Package => ScopeKind::Package,
            },
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
