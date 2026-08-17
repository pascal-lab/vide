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
    module::ModuleKind,
    owner::{OwnerId, OwnerKind},
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnitData {
    file: HirFileId,
    name: SmolStr,
    kind: UnitKind,
    top_level: bool,
    ordinal: u32,
}

/// File-level design-unit declarations, independent of lexical `ScopeGraph`.
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
    pub fn module_ids(&self, db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(db, name, |unit| unit.kind.is_module())
    }

    /// Design-unit modules declared at compilation-unit scope. Only these may
    /// act as explicit hierarchy roots for multi-segment paths.
    pub fn top_level_module_ids(&self, db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(db, name, |unit| unit.kind.is_module() && unit.top_level)
    }

    pub fn package_ids(&self, db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(db, name, |unit| unit.kind.is_package())
    }

    /// Resolve an instance target using the containing module's local
    /// checker/covergroup declarations before compilation-unit declarations.
    pub fn instantiable_ids_in(
        &self,
        db: &dyn HirDefDb,
        scope: OwnerId,
        name: &SmolStr,
    ) -> Resolution<OwnerId> {
        let file_id = scope.file(db);
        let local = Resolution::from_candidates(
            db.owner_table(file_id)
                .owners()
                .iter()
                .filter(|owner| {
                    owner.parent == Some(scope)
                        && owner.name == *name
                        && matches!(owner.kind, OwnerKind::Checker | OwnerKind::Covergroup)
                })
                .map(|owner| owner.id),
        );
        if !local.is_unresolved() {
            return local;
        }
        self.resolve(db, name, |unit| {
            unit.kind.is_instantiable() && (unit.kind.is_module() || unit.top_level)
        })
    }

    pub fn module_names(&self) -> impl Iterator<Item = &SmolStr> {
        self.module_names.iter()
    }

    fn resolve(
        &self,
        db: &dyn HirDefDb,
        name: &SmolStr,
        matches: impl Fn(&UnitData) -> bool,
    ) -> Resolution<OwnerId> {
        let candidates =
            self.by_name.get(name).into_iter().flat_map(|indices| indices.iter()).filter_map(
                |index| {
                    let unit = self.units.get(*index)?;
                    matches(unit).then(|| locate_unit_owner(db, unit)).flatten()
                },
            );
        Resolution::from_candidates(candidates)
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub fn unit_index(db: &dyn HirDefDb) -> Arc<UnitIndex> {
    let mut index = UnitIndex::default();

    for file_id in db
        .files()
        .iter()
        .copied()
        .filter(|&file_id| db.file_kind(file_id).is_semantic_compilation_unit())
    {
        let Some(skeleton) = db.declaration_skeleton(HirFileId::File(file_id)) else {
            continue;
        };
        add_file_units(&mut index, HirFileId::File(file_id), skeleton.item_tree());
        for macro_file in preproc_expand::macro_file::macro_files_for_file(db, file_id) {
            add_file_units(
                &mut index,
                HirFileId::Macro(macro_file),
                &db.item_tree(HirFileId::Macro(macro_file)),
            );
        }
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

fn add_file_units(index: &mut UnitIndex, file: HirFileId, item_tree: &crate::item_tree::ItemTree) {
    let file_owner = item_tree.root_owner();
    for header in item_tree.module_headers() {
        let owner = item_tree.owners().owner(header.owner());
        insert_unit(
            index,
            file,
            header.name().clone(),
            UnitKind::Module(header.kind()),
            owner.is_some_and(|data| data.parent == file_owner),
        );
    }
    for owner in item_tree.owners().owners() {
        let kind = match owner.kind {
            OwnerKind::Checker => UnitKind::Checker,
            OwnerKind::Covergroup => UnitKind::Covergroup,
            _ => continue,
        };
        insert_unit(index, file, owner.name.clone(), kind, owner.parent == file_owner);
    }
}

fn insert_unit(
    index: &mut UnitIndex,
    file: HirFileId,
    name: SmolStr,
    kind: UnitKind,
    top_level: bool,
) {
    if name.is_empty() {
        return;
    }
    let ordinal = index
        .units
        .iter()
        .filter(|unit| unit.file == file && unit.name == name && unit.kind == kind)
        .count() as u32;
    let unit_index = index.units.len();
    index.units.push(UnitData { file, name: name.clone(), kind, top_level, ordinal });
    index.by_name.entry(name).or_default().push(unit_index);
}

fn locate_unit_owner(db: &dyn HirDefDb, unit: &UnitData) -> Option<OwnerId> {
    let table = db.owner_table(unit.file);
    table
        .owners()
        .iter()
        .filter(|owner| {
            owner.name == unit.name
                && owner_matches_unit_kind(owner.kind, owner.module_kind, unit.kind)
        })
        .nth(unit.ordinal as usize)
        .map(|owner| owner.id)
}

fn owner_matches_unit_kind(
    owner_kind: OwnerKind,
    module_kind: Option<ModuleKind>,
    unit_kind: UnitKind,
) -> bool {
    match (owner_kind, unit_kind) {
        (OwnerKind::Module, UnitKind::Module(kind)) => module_kind == Some(kind),
        (OwnerKind::Checker, UnitKind::Checker) | (OwnerKind::Covergroup, UnitKind::Covergroup) => {
            true
        }
        _ => false,
    }
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
        assert_eq!(index.module_names().count(), 0);
        assert!(index.by_name.is_empty());
    }
}
