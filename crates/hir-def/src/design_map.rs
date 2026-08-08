//! Canonical design-unit export graph.
//!
//! A package's exported namespace is the union of its own declarations and
//! the declarations made visible by its package imports.  This module owns
//! that graph so package imports are resolved consistently for both direct
//! package queries and lexical name resolution.

use base_db::salsa;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use triomphe::Arc;

use crate::{
    db::HirDefDb,
    def_id::DefId,
    owner::OwnerId,
    symbol::{Import, NameContext, NameScope, Resolution},
};
/// Fixed-point package exports for one database revision.
///
/// The map contains only package owners.  Every stored scope is import-free:
/// package imports have already been projected into its type/value namespace.
/// This makes the result the single source of truth for package exports and
/// avoids recursive resolver calls for nested or cyclic imports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignMap {
    package_exports: FxHashMap<OwnerId, Arc<NameScope>>,
}

impl DesignMap {
    pub fn package_export_scope(&self, package: OwnerId) -> Option<Arc<NameScope>> {
        self.package_exports.get(&package).cloned()
    }

    /// Resolve one package import while preserving ambiguous package parents.
    pub fn resolve_import(
        &self,
        db: &dyn HirDefDb,
        import: &Import,
        ident: &SmolStr,
        ctx: NameContext,
    ) -> Resolution<DefId> {
        if let Some(imported_name) = &import.name
            && imported_name != ident
        {
            return Resolution::Unresolved;
        }

        let packages = db.unit_scope().package_ids(db, &import.package);
        packages.and_then(|package| {
            let Some(exports) = self.package_exports.get(&package) else {
                return Resolution::Unresolved;
            };
            exports.lookup(ctx, ident)
        })
    }
}

#[salsa::tracked(lru = 128, returns(clone))]
pub fn design_map(db: &dyn HirDefDb) -> Arc<DesignMap> {
    let mut packages = db
        .files()
        .iter()
        .flat_map(|file_id| {
            db.item_tree(HirFileId::File(*file_id))
                .module_headers()
                .filter(|header| header.kind() == crate::module::ModuleKind::Package)
                .map(|header| header.owner())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    let unit_scope = db.unit_scope();

    let mut exports = FxHashMap::default();
    for package in &packages {
        exports.insert(*package, db.scope_for(*package).without_imports());
    }

    // Each iteration can only add definitions to an export scope.  The
    // finite namespace therefore converges, including mutually importing
    // packages, without a recursion limit or a silent cycle fallback.
    loop {
        let mut changed = false;
        for package in &packages {
            let local_scope = db.scope_for(*package);
            let mut next = exports
                .get(package)
                .expect("every package must have an initial export scope")
                .clone();

            for import in local_scope.imports() {
                match &import.name {
                    Some(imported_name) => {
                        for ctx in [NameContext::Type, NameContext::Value] {
                            let resolution = resolve_package_member(
                                &exports,
                                unit_scope.package_ids(db, &import.package),
                                imported_name,
                                ctx,
                            );
                            next.insert_resolution(ctx, imported_name, resolution);
                        }
                    }
                    None => {
                        let names =
                            imported_names(&exports, unit_scope.package_ids(db, &import.package));
                        for name in names {
                            for ctx in [NameContext::Type, NameContext::Value] {
                                let resolution = resolve_package_member(
                                    &exports,
                                    unit_scope.package_ids(db, &import.package),
                                    &name,
                                    ctx,
                                );
                                next.insert_resolution(ctx, &name, resolution);
                            }
                        }
                    }
                }
            }

            if next != *exports.get(package).expect("package export scope exists") {
                exports.insert(*package, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    Arc::new(DesignMap {
        package_exports: exports
            .into_iter()
            .map(|(owner, scope)| (owner, Arc::new(scope)))
            .collect(),
    })
}

fn resolve_package_member(
    exports: &FxHashMap<OwnerId, NameScope>,
    packages: Resolution<OwnerId>,
    name: &SmolStr,
    ctx: NameContext,
) -> Resolution<DefId> {
    packages.and_then(|package| {
        exports
            .get(&package)
            .expect("package resolution must have a design-map entry")
            .lookup(ctx, name)
    })
}

fn imported_names(
    exports: &FxHashMap<OwnerId, NameScope>,
    packages: Resolution<OwnerId>,
) -> Vec<SmolStr> {
    let mut names = Vec::new();
    for package in packages.iter() {
        let scope = exports.get(package).expect("package resolution must have a design-map entry");
        for (name, _) in scope.iter_listing() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    names
}

pub(crate) fn set_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    design_map::set_lru_capacity(db, capacity);
}
