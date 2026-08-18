//! Two-clock incrementality for workspace products.
//!
//! Salsa tracks per-file queries. This module tracks workspace-sized values
//! that must not enter the Salsa dependency graph — notably
//! [`hir_def::pathres::ResolutionContext`] and [`design_graph::UnitCatalog`].
//! Once a per-file query reads `unit_scope` through Salsa, every file hangs
//! off the whole project.
//!
//! Two clocks:
//! - Salsa revision `r` — any input change
//! - Structure epoch `s` — a dirty file's L0 compilation-unit declarations
//!   changed
//!
//! T10 measured a salsa `source_unit_catalog` over position-free `file_decls`.
//! After a body-only edit the decls are value-equal, but salsa still
//! re-executes the workspace catalog (see
//! `file_decls_backdate_across_a_body_only_edit`). The fold is cheap for
//! decls, but generated-unit overlay and parse-dependency edges are not
//! salsa inputs, so epoch remains the invalidation barrier. This is a
//! measurement, not a theory.
//!
//! Structure products (`UnitCatalog`, `ResolutionContext`) are keyed by `s`
//! and memoized in `ProductCell` so a foreground request can preempt a
//! background prewarm. A generated-unit set change patches the graph for that
//! file via [`ProductStore::patch_design_graph`]. Generated units are stored
//! under `(FileId, compilation_unit_snapshot.fingerprint)` so a later
//! snapshot cannot observe a previous artifact's names. Making them a salsa
//! query over `compilation_unit_artifact` would force a paid parse of every
//! previously-parsed CU on the next fold; that undoes the L0 fact layer.
//!
//! [`ProductStore::invalidate`] is the only epoch-decision entry point.
//! Features are pure functions of [`crate::analysis::AnalysisContext`],
//! except that a paid parse may publish fingerprint-keyed generated units
//! onto the already-decided graph.
//!
//! New caches belong in Salsa (per-file, dependency-tracked) or in
//! [`ProductStore`] (workspace-scoped, epoch-tracked). A third cache in a
//! feature function or on `RootDb` is a bug.

mod epoch;
mod product_cell;
mod store;

pub(crate) use product_cell::ComputationPriority;
pub(crate) use store::ProductStore;
