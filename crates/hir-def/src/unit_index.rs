//! File-level index of design-unit declarations.
//!
//! This index owns module-like and instantiable design-unit headers. It
//! deliberately does not build a lexical scope, lower a body, or allocate a
//! `DefId`; callers choose when to project an indexed owner into a semantic
//! definition.

use base_db::salsa;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use smol_str::SmolStr;
use triomphe::Arc;

use crate::{
    db::HirDefDb,
    item_tree::ItemTree,
    module::ModuleKind,
    owner::{OwnerId, OwnerKind, OwnerTable},
    symbol::Resolution,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitKind {
    Module(ModuleKind),
    Checker,
    Covergroup,
}

impl UnitKind {
    fn is_module(self) -> bool {
        matches!(self, Self::Module(kind) if kind.is_instantiable())
    }

    fn is_package(self) -> bool {
        matches!(self, Self::Module(ModuleKind::Package))
    }

    fn is_instantiable(self) -> bool {
        self.is_module() || matches!(self, Self::Checker | Self::Covergroup)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UnitData {
    owner: OwnerId,
    kind: UnitKind,
    parent: Option<OwnerId>,
    top_level: bool,
}

/// File-level design-unit declarations, independent of lexical `NameScope`.
///
/// The index is built from [`crate::item_tree::ItemTree::module_headers`] and
/// structural owner metadata for checker/covergroup declarations. It preserves
/// duplicate declarations as `Resolution::Ambiguous`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UnitIndex {
    units: Vec<UnitData>,
    by_name: FxHashMap<SmolStr, SmallVec<[usize; 2]>>,
    module_names: Vec<SmolStr>,
}
impl UnitIndex {
    pub fn module_ids(&self, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(name, |unit| unit.kind.is_module())
    }

    pub fn package_ids(&self, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(name, |unit| unit.kind.is_package())
    }

    /// Resolve an instance target using the containing module's local
    /// checker/covergroup declarations before compilation-unit declarations.
    pub fn instantiable_ids_in(&self, scope: OwnerId, name: &SmolStr) -> Resolution<OwnerId> {
        let local = self.resolve(name, |unit| {
            matches!(unit.kind, UnitKind::Checker | UnitKind::Covergroup)
                && unit.parent == Some(scope)
        });
        if !local.is_unresolved() {
            return local;
        }
        self.resolve(name, |unit| {
            unit.kind.is_instantiable() && (unit.kind.is_module() || unit.top_level)
        })
    }

    pub fn module_names(&self) -> impl Iterator<Item = &SmolStr> {
        self.module_names.iter()
    }

    fn resolve(&self, name: &SmolStr, matches: impl Fn(&UnitData) -> bool) -> Resolution<OwnerId> {
        let candidates =
            self.by_name.get(name).into_iter().flat_map(|indices| indices.iter()).filter_map(
                |index| {
                    let unit = self.units.get(*index)?;
                    matches(unit).then_some(unit.owner)
                },
            );
        Resolution::from_candidates(candidates)
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub fn unit_index(db: &dyn HirDefDb) -> Arc<UnitIndex> {
    let mut index = UnitIndex::default();

    for file_id in db.files().iter() {
        let file_id = HirFileId::File(*file_id);
        let item_tree = db.item_tree(file_id);
        let owner_table = db.owner_table(file_id);
        add_file_units(&mut index, &item_tree, &owner_table);
    }

    index.module_names = index
        .by_name
        .iter()
        .filter_map(|(name, indices)| {
            indices
                .iter()
                .any(|unit_index| {
                    index.units.get(*unit_index).is_some_and(|unit| unit.kind.is_module())
                })
                .then_some(name.clone())
        })
        .collect();
    index.module_names.sort();
    index.module_names.dedup();

    Arc::new(index)
}

fn add_file_units(index: &mut UnitIndex, item_tree: &ItemTree, owner_table: &OwnerTable) {
    let file_owner = owner_table.file_owner().expect("owner table must contain its file owner");
    for header in item_tree.module_headers() {
        let owner = header.owner();
        let data = owner_table.owner(owner).expect("module header owner must be indexed");
        insert_unit(
            index,
            header.name().clone(),
            owner,
            UnitKind::Module(header.kind()),
            data.parent,
            data.parent == Some(file_owner),
        );
    }
    for owner in owner_table.owners() {
        let kind = match owner.kind {
            OwnerKind::Checker => UnitKind::Checker,
            OwnerKind::Covergroup => UnitKind::Covergroup,
            _ => continue,
        };
        insert_unit(
            index,
            owner.name.clone(),
            owner.id,
            kind,
            owner.parent,
            owner.parent == Some(file_owner),
        );
    }
}

fn insert_unit(
    index: &mut UnitIndex,
    name: SmolStr,
    owner: OwnerId,
    kind: UnitKind,
    parent: Option<OwnerId>,
    top_level: bool,
) {
    if name.is_empty() {
        return;
    }
    let unit_index = index.units.len();
    index.units.push(UnitData { owner, kind, parent, top_level });
    index.by_name.entry(name).or_default().push(unit_index);
}

pub(crate) fn set_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    unit_index::set_lru_capacity(db, capacity);
}
#[cfg(test)]
mod tests {
    use super::UnitIndex;

    #[test]
    fn empty_index_has_no_targets() {
        let index = UnitIndex::default();
        assert!(index.module_ids(&"missing".into()).is_unresolved());
        assert!(index.package_ids(&"missing".into()).is_unresolved());
        assert_eq!(index.module_names().count(), 0);
    }
}
