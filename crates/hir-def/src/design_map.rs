//! Canonical design-unit export graph.
//!
//! A package's exported namespace is the union of its own declarations and
//! the declarations made visible by its package imports.  This module owns
//! that graph so package imports are resolved consistently for both direct
//! package queries and lexical name resolution.

use std::cell::Cell;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::get::GetRef;

use crate::{
    Ident, PackageExport,
    body::BodyItem,
    container::OwnerRef,
    db::HirDefDb,
    def_id::DefId,
    owner::OwnerId,
    stmt::StmtKind,
    symbol::{Import, NameContext, Resolution},
};

/// The import-free declarations exported directly by one package.
///
/// Package export resolution owns this namespace instead of borrowing the
/// lexical `ScopeGraph`. The latter represents a scope chain and import edges;
/// this value represents only the package's public type/value bindings.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageExports {
    types: FxHashMap<SmolStr, SmallVec<[DefId; 1]>>,
    values: FxHashMap<SmolStr, SmallVec<[DefId; 1]>>,
    assertions: FxHashMap<SmolStr, SmallVec<[DefId; 1]>>,
}

impl PackageExports {
    pub fn lookup(&self, ctx: NameContext, ident: &Ident) -> Resolution<DefId> {
        let candidates = match ctx {
            NameContext::Type => self.types.get(ident).map(SmallVec::as_slice).unwrap_or_default(),
            NameContext::Value => {
                self.values.get(ident).map(SmallVec::as_slice).unwrap_or_default()
            }
            NameContext::Assertion => {
                self.assertions.get(ident).map(SmallVec::as_slice).unwrap_or_default()
            }
            NameContext::Listing => {
                return Resolution::from_candidates(self.lookup_listing(ident));
            }
        };
        Resolution::from_candidates(candidates.iter().copied())
    }

    pub fn lookup_listing(&self, ident: &Ident) -> SmallVec<[DefId; 1]> {
        let mut defs = SmallVec::new();
        if let Some(type_defs) = self.types.get(ident) {
            defs.extend(type_defs.iter().copied());
        }
        if let Some(value_defs) = self.values.get(ident) {
            defs.extend(value_defs.iter().copied());
        }
        if let Some(assertion_defs) = self.assertions.get(ident) {
            defs.extend(assertion_defs.iter().copied());
        }
        defs
    }

    pub fn iter_listing(&self) -> impl Iterator<Item = (&SmolStr, SmallVec<[DefId; 1]>)> + '_ {
        self.types
            .iter()
            .map(|(ident, type_defs)| {
                let mut defs = type_defs.iter().copied().collect::<SmallVec<[DefId; 1]>>();
                if let Some(value_defs) = self.values.get(ident) {
                    defs.extend(value_defs.iter().copied());
                }
                if let Some(assertion_defs) = self.assertions.get(ident) {
                    defs.extend(assertion_defs.iter().copied());
                }
                (ident, defs)
            })
            .chain(self.values.iter().filter(|(ident, _)| !self.types.contains_key(*ident)).map(
                |(ident, defs)| {
                    let mut all = defs.iter().copied().collect::<SmallVec<[DefId; 1]>>();
                    if let Some(assertion_defs) = self.assertions.get(ident) {
                        all.extend(assertion_defs.iter().copied());
                    }
                    (ident, all)
                },
            ))
            .chain(
                self.assertions
                    .iter()
                    .filter(|(ident, _)| {
                        !self.types.contains_key(*ident) && !self.values.contains_key(*ident)
                    })
                    .map(|(ident, defs)| (ident, defs.iter().copied().collect())),
            )
    }

    fn insert_type(&mut self, ident: &Ident, def_id: DefId) {
        insert_binding(&mut self.types, ident, def_id);
    }

    fn insert_assertion(&mut self, ident: &Ident, def_id: DefId) {
        insert_binding(&mut self.assertions, ident, def_id);
    }

    fn insert_value(&mut self, ident: &Ident, def_id: DefId) {
        insert_binding(&mut self.values, ident, def_id);
    }

    fn insert_resolution(
        &mut self,
        ctx: NameContext,
        ident: &Ident,
        resolution: Resolution<DefId>,
    ) {
        for def_id in resolution.into_candidates() {
            match ctx {
                NameContext::Type => self.insert_type(ident, def_id),
                NameContext::Value => self.insert_value(ident, def_id),
                NameContext::Assertion => self.insert_assertion(ident, def_id),
                NameContext::Listing => {
                    self.insert_type(ident, def_id);
                    self.insert_value(ident, def_id);
                }
            }
        }
    }
}

fn insert_binding(
    bindings: &mut FxHashMap<SmolStr, SmallVec<[DefId; 1]>>,
    ident: &Ident,
    def_id: DefId,
) {
    let defs = bindings.entry(ident.clone()).or_default();
    if !defs.contains(&def_id) {
        defs.push(def_id);
    }
}

fn package_bindings(db: &dyn HirDefDb, package: OwnerId) -> PackageExports {
    let body = db.body(package);
    let body_scope = body.scope(package).expect("package body must have a root scope");
    let mut exports = PackageExports::default();

    for &decl_id in body_scope.declarators() {
        let declaration = &body.decls[decl_id];
        if let Some(name) = &declaration.name {
            exports.insert_value(name, DefId::from_source(db, OwnerRef::new(package, decl_id)));
        }
    }
    for &typedef_id in body_scope.typedefs() {
        let typedef = &body.typedefs[typedef_id];
        if let Some(name) = &typedef.name {
            exports.insert_type(name, DefId::from_source(db, OwnerRef::new(package, typedef_id)));
        }
    }
    for subroutine_owner in body.subroutine_owners() {
        let subroutine = db.subroutine(subroutine_owner);
        if let Some(name) = &subroutine.name {
            exports.insert_value(
                name,
                DefId::from_source(db, crate::symbol::DefOriginLoc::Subroutine(subroutine_owner)),
            );
        }
    }
    for &stmt_id in body_scope.statements() {
        let statement = &body.stmts[stmt_id];
        if let Some(label) = &statement.label {
            exports.insert_value(label, DefId::from_source(db, OwnerRef::new(package, stmt_id)));
        }
        if let StmtKind::Block(block_owner) = statement.kind
            && let Some(name) = block_owner.name(db)
        {
            exports.insert_value(
                &name,
                DefId::from_source(db, crate::symbol::DefOriginLoc::Block(block_owner)),
            );
        }
    }
    for item in &body.items {
        match item {
            BodyItem::CheckerOwner(owner) => {
                let Some(origin) = owner.as_checker(db) else { continue };
                let name = body.get(origin.value).name.clone();
                if let Some(name) = name {
                    exports.insert_type(&name, DefId::from_source(db, origin));
                }
            }
            BodyItem::CovergroupOwner(owner) => {
                let Some(origin) = owner.as_covergroup(db) else { continue };
                let name = body.get(origin.value).name.clone();
                if let Some(name) = name {
                    exports.insert_type(&name, DefId::from_source(db, origin));
                }
            }
            BodyItem::PropertyId(property_id) => {
                let property = body.get(*property_id);
                if let Some(name) = &property.name {
                    exports.insert_assertion(
                        name,
                        DefId::from_source(db, OwnerRef::new(package, *property_id)),
                    );
                }
            }
            BodyItem::SequenceId(sequence_id) => {
                let sequence = body.get(*sequence_id);
                if let Some(name) = &sequence.name {
                    exports.insert_assertion(
                        name,
                        DefId::from_source(db, OwnerRef::new(package, *sequence_id)),
                    );
                }
            }
            BodyItem::ClockingBlockOwner(owner) => {
                let Some(origin) = owner.as_clocking_block(db) else { continue };
                let name = body.get(origin.value).name.clone();
                if let Some(name) = name {
                    exports.insert_value(&name, DefId::from_source(db, origin));
                }
            }
            _ => {}
        }
    }
    exports
}
/// Fixed-point package exports for one database revision.
///
/// The map contains only package owners. Every stored namespace contains
/// direct declarations plus package-import bindings; lexical `ScopeGraph`
/// construction is not part of this query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignMap {
    package_exports: FxHashMap<OwnerId, Arc<PackageExports>>,
}

impl DesignMap {
    pub fn package_exports(&self, package: OwnerId) -> Option<Arc<PackageExports>> {
        self.package_exports.get(&package).cloned()
    }

    /// Resolve one package import while preserving ambiguous package parents.
    pub fn resolve_import(
        &self,
        db: &dyn HirDefDb,
        graph: &design_graph::UnitCatalog,
        import: &Import,
        ident: &SmolStr,
        ctx: NameContext,
    ) -> Resolution<DefId> {
        if let Some(imported_name) = &import.name
            && imported_name != ident
        {
            return Resolution::Unresolved;
        }

        let packages = Resolution::from_candidates(
            graph
                .packages_named(&import.package)
                .into_vec()
                .into_iter()
                .filter_map(|unit| crate::unit::ToOwner::to_owner(unit, db)),
        );
        packages.and_then(|package| {
            let Some(exports) = self.package_exports.get(&package) else {
                return Resolution::Unresolved;
            };
            exports.lookup(ctx, ident)
        })
    }
}

thread_local! {
    /// Executions of [`package_export_closure`]. A salsa memo must keep this
    /// at one per request, not one per `resolution()` / `semantics()` call.
    pub static PACKAGE_EXPORT_CLOSURE_RUNS: Cell<u32> = const { Cell::new(0) };
    /// Paid [`ToOwner::to_owner`] calls performed while building the closure.
    pub static PACKAGE_EXPORT_TO_OWNER_RUNS: Cell<u32> = const { Cell::new(0) };
}

/// Closed package-export graph for the packages on `graph`.
pub fn package_export_closure(
    db: &dyn HirDefDb,
    graph: &design_graph::UnitCatalog,
) -> Arc<DesignMap> {
    PACKAGE_EXPORT_CLOSURE_RUNS.with(|runs| runs.set(runs.get() + 1));
    let mut packages: Vec<OwnerId> = graph
        .packages()
        .filter_map(|unit| {
            PACKAGE_EXPORT_TO_OWNER_RUNS.with(|runs| runs.set(runs.get() + 1));
            crate::unit::ToOwner::to_owner(unit, db)
        })
        .collect();
    packages.sort();
    packages.dedup();

    let mut exports = FxHashMap::default();
    let mut imports = FxHashMap::default();
    let mut reexports = FxHashMap::default();
    for package in &packages {
        let body = db.body(*package);
        exports.insert(*package, package_bindings(db, *package));
        imports.insert(
            *package,
            body.package_imports
                .values()
                .map(|import| Import {
                    package: import.package.clone(),
                    name: import.item.clone(),
                    source: None,
                })
                .collect::<Vec<_>>(),
        );
        reexports.insert(*package, body.package_exports.values().cloned().collect::<Vec<_>>());
    }

    // Each iteration can only add definitions to an export namespace. The
    // finite namespace therefore converges, including mutually importing
    // packages, without a recursion limit or a silent cycle fallback.
    loop {
        let mut changed = false;
        for package in &packages {
            let mut next = exports
                .get(package)
                .expect("every package must have an initial export namespace")
                .clone();

            let mut add_reexport = |source_package: &Ident, item: Option<&Ident>| {
                let source_owners = Resolution::from_candidates(
                    graph
                        .packages_named(source_package)
                        .into_vec()
                        .into_iter()
                        .filter_map(|unit| crate::unit::ToOwner::to_owner(unit, db)),
                );
                let names = item
                    .map(|item| vec![item.clone()])
                    .unwrap_or_else(|| imported_names(&exports, source_owners.clone()));
                for name in names {
                    for ctx in [NameContext::Type, NameContext::Value, NameContext::Assertion] {
                        let resolution =
                            resolve_package_member(&exports, source_owners.clone(), &name, ctx);
                        next.insert_resolution(ctx, &name, resolution);
                    }
                }
            };

            for export in reexports.get(package).expect("every package has export edges") {
                match export {
                    PackageExport::Package { package, item, .. } => {
                        add_reexport(package, item.as_ref());
                    }
                    PackageExport::All { .. } => {
                        for import in imports.get(package).expect("every package has import edges")
                        {
                            add_reexport(&import.package, None);
                        }
                    }
                }
            }

            if next != *exports.get(package).expect("package export namespace exists") {
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
            .map(|(owner, exports)| (owner, Arc::new(exports)))
            .collect(),
    })
}

fn resolve_package_member(
    exports: &FxHashMap<OwnerId, PackageExports>,
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
    exports: &FxHashMap<OwnerId, PackageExports>,
    packages: Resolution<OwnerId>,
) -> Vec<SmolStr> {
    let mut names = Vec::new();
    for package in packages.iter() {
        let package_exports =
            exports.get(package).expect("package resolution must have a design-map entry");
        for (name, _) in package_exports.iter_listing() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    names.sort();
    names
}
