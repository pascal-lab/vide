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
//! Three product kinds:
//! - **Structure products** (`DesignGraph`, `ResolutionContext`,
//!   `SemanticSnapshotInputs`): keyed by `s`, memoized in `ProductCell` so a
//!   foreground request can preempt a background prewarm. A generated-unit set
//!   change also drops these three cells via
//!   [`ProductStore::invalidate_design_graph`].
//! - **File shards** (`FileNameIndex`, `FileModuleEdges`): keyed by
//!   `(generation, FileId)` against a single per-file generation clock
//! - **Merged indexes** (`NameIndex`, `ModuleEdgeIndex`): folds over shards; a
//!   Drop epoch forces a full rebuild
//!
//! [`ProductStore::invalidate`] is the only invalidation entry point.
//! Features are pure functions of [`crate::analysis::AnalysisContext`].
//!
//! New caches belong in Salsa (per-file, dependency-tracked) or in
//! [`ProductStore`] (workspace-scoped, epoch-tracked). A third cache in a
//! feature function or on `RootDb` is a bug.

mod epoch;
mod indexes;
mod product_cell;
mod store;

pub(crate) use product_cell::ComputationPriority;
pub(crate) use store::ProductStore;
