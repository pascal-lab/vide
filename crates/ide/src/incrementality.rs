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
//! file via [`ProductStore::patch_design_graph`].
//!
//! [`ProductStore::invalidate`] is the only invalidation entry point.
//! Features are pure functions of [`crate::analysis::AnalysisContext`].
//!
//! New caches belong in Salsa (per-file, dependency-tracked) or in
//! [`ProductStore`] (workspace-scoped, epoch-tracked). A third cache in a
//! feature function or on `RootDb` is a bug.

mod epoch;
mod product_cell;
mod store;

pub(crate) use product_cell::ComputationPriority;
pub(crate) use store::ProductStore;
