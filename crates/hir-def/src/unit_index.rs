//! File-level index of design-unit declarations.
//!
//! This index owns module-like and instantiable design-unit headers. It
//! deliberately does not build a lexical scope, lower a body, or allocate a
//! `DefId`; callers choose when to project an indexed owner into a semantic
//! definition.

use base_db::salsa;
use preproc_expand::{file::HirFileId, macro_file::macro_files_for_file};
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
        let declared = self.resolve(db, name, |unit| unit.kind.is_module());
        if !declared.is_unresolved() {
            return declared;
        }
        // Macro-generated modules are not CU decls in the unexpanded shard.
        // L2 only the files that mention the spelling.
        locate_modules_in_mentioning_files(db, name)
    }

    /// Design-unit modules declared at compilation-unit scope. Only these may
    /// act as explicit hierarchy roots for multi-segment paths.
    pub fn top_level_module_ids(&self, db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(db, name, |unit| unit.kind.is_module() && unit.top_level)
    }

    pub fn package_ids(&self, db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
        self.resolve(db, name, |unit| unit.kind.is_package())
    }

    /// Package owners declared in the L0 index. Locating them is L2 of those
    /// files only — not every compilation unit.
    pub fn package_owners(&self, db: &dyn HirDefDb) -> Vec<OwnerId> {
        self.units
            .iter()
            .filter(|unit| unit.kind.is_package())
            .filter_map(|unit| locate_unit_owner(db, unit))
            .collect()
    }

    /// Modules, packages, checkers, and covergroups visible as `$unit` types.
    pub fn type_unit_ids(&self, db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
        let declared = self.resolve(db, name, |unit| {
            unit.kind.is_module()
                || unit.kind.is_package()
                || matches!(unit.kind, UnitKind::Checker | UnitKind::Covergroup)
        });
        if !declared.is_unresolved() {
            return declared;
        }
        locate_modules_in_mentioning_files(db, name)
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

    /// Whether this compilation-unit declaration is a candidate for
    /// instantiations of `name`. Identity is the L0 record (file, name,
    /// kind, ordinal), not a lowered `OwnerId`.
    pub fn declares_instantiable(
        &self,
        file_id: vfs::FileId,
        name: &str,
        role: crate::decl_shard::DeclRole,
        ordinal: u32,
    ) -> bool {
        let Some(kind) = unit_kind_from_role(role) else {
            return false;
        };
        let Some(kind) = instantiable_kind(kind) else {
            return false;
        };
        self.by_name.get(name).into_iter().flatten().any(|&index| {
            self.units.get(index).is_some_and(|unit| {
                unit.file == HirFileId::File(file_id)
                    && unit.name == name
                    && unit.kind == kind
                    && unit.ordinal == ordinal
            })
        })
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
        add_file_units(&mut index, HirFileId::File(file_id), &db.file_decl_shard(file_id));
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

fn unit_kind_from_role(role: crate::decl_shard::DeclRole) -> Option<UnitKind> {
    Some(match role {
        crate::decl_shard::DeclRole::Module => UnitKind::Module(ModuleKind::Module),
        crate::decl_shard::DeclRole::Interface => UnitKind::Module(ModuleKind::Interface),
        crate::decl_shard::DeclRole::Package => UnitKind::Module(ModuleKind::Package),
        crate::decl_shard::DeclRole::Program => UnitKind::Module(ModuleKind::Program),
        crate::decl_shard::DeclRole::Checker => UnitKind::Checker,
        crate::decl_shard::DeclRole::Covergroup => UnitKind::Covergroup,
        _ => return None,
    })
}

fn instantiable_kind(kind: UnitKind) -> Option<UnitKind> {
    kind.is_instantiable().then_some(kind)
}

fn add_file_units(
    index: &mut UnitIndex,
    file: HirFileId,
    shard: &crate::decl_shard::FileDeclShard,
) {
    for decl in shard.decls.iter() {
        let Some(kind) = unit_kind_from_role(decl.role) else {
            continue;
        };
        insert_unit(index, file, decl.name.clone(), kind, true);
    }
}

fn locate_modules_in_mentioning_files(db: &dyn HirDefDb, name: &SmolStr) -> Resolution<OwnerId> {
    let files: Vec<_> = db.files().iter().copied().collect();
    let candidates = files.into_iter().filter_map(|file_id| {
        if !db.file_kind(file_id).is_semantic_compilation_unit() {
            return None;
        }
        if !db.file_decl_shard(file_id).mentions_name(name) {
            return None;
        }
        locate_named_instantiable_module(db, file_id, name)
    });
    Resolution::from_candidates(candidates)
}

fn locate_named_instantiable_module(
    db: &dyn HirDefDb,
    file_id: vfs::FileId,
    name: &SmolStr,
) -> Option<OwnerId> {
    let is_match = |owner: &crate::owner::OwnerData| {
        owner.name == *name
            && owner.kind == OwnerKind::Module
            && owner.module_kind.is_some_and(|kind| kind.is_instantiable())
    };
    for macro_file in macro_files_for_file(db, file_id) {
        if let Some(owner) = db
            .owner_table(HirFileId::Macro(macro_file))
            .owners()
            .iter()
            .find_map(|owner| is_match(owner).then_some(owner.id))
        {
            return Some(owner);
        }
    }
    db.owner_table(HirFileId::File(file_id))
        .owners()
        .iter()
        .find_map(|owner| is_match(owner).then_some(owner.id))
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
    let file_id = match unit.file {
        HirFileId::File(file_id) => file_id,
        HirFileId::Macro(_) => {
            return matching_unit_owners(db, unit.file, unit)
                .into_iter()
                .nth(unit.ordinal as usize);
        }
    };
    let macro_owners: Vec<OwnerId> = macro_files_for_file(db, file_id)
        .into_iter()
        .flat_map(|macro_file| matching_unit_owners(db, HirFileId::Macro(macro_file), unit))
        .collect();
    if !macro_owners.is_empty() {
        return macro_owners.into_iter().nth(unit.ordinal as usize);
    }
    matching_unit_owners(db, HirFileId::File(file_id), unit).into_iter().nth(unit.ordinal as usize)
}

fn matching_unit_owners(db: &dyn HirDefDb, file: HirFileId, unit: &UnitData) -> Vec<OwnerId> {
    db.owner_table(file)
        .owners()
        .iter()
        .filter(|owner| {
            owner.name == unit.name
                && owner_matches_unit_kind(owner.kind, owner.module_kind, unit.kind)
        })
        .map(|owner| owner.id)
        .collect()
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
    use preproc_expand::file::HirFileId;

    use super::{UnitIndex, UnitKind, insert_unit};
    use crate::{decl_shard::DeclRole, module::ModuleKind};

    #[test]
    fn empty_index_has_no_targets() {
        let index = UnitIndex::default();
        assert_eq!(index.module_names().count(), 0);
        assert!(index.by_name.is_empty());
    }

    #[test]
    fn declares_instantiable_is_the_l0_record() {
        let mut index = UnitIndex::default();
        let file = vfs::FileId::from_raw(1);
        insert_unit(
            &mut index,
            HirFileId::File(file),
            "fifo".into(),
            UnitKind::Module(ModuleKind::Module),
            true,
        );
        insert_unit(
            &mut index,
            HirFileId::File(file),
            "fifo".into(),
            UnitKind::Module(ModuleKind::Package),
            true,
        );
        assert!(index.declares_instantiable(file, "fifo", DeclRole::Module, 0));
        assert!(!index.declares_instantiable(file, "fifo", DeclRole::Module, 1));
        assert!(!index.declares_instantiable(file, "fifo", DeclRole::Package, 0));
        assert!(!index.declares_instantiable(
            vfs::FileId::from_raw(2),
            "fifo",
            DeclRole::Module,
            0
        ));
    }
}
