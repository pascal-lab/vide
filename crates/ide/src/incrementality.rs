//! Two-clock incrementality for workspace products.
//!
//! Salsa tracks per-file queries. This module tracks workspace-sized values
//! that must not enter the Salsa dependency graph — notably
//! [`hir_def::pathres::ResolutionContext`] and [`design_graph::DesignGraph`].
//! Once a per-file query reads `unit_scope` through Salsa, every file hangs
//! off the whole project.
//!
//! Two clocks:
//! - Salsa revision `r` — any input change
//! - Structure epoch `s` — a dirty file's L0 compilation-unit declarations
//!   changed
//!
//! Structure products (`DesignGraph`, `ResolutionContext`) are keyed by `s`
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
