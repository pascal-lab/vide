//! Two-clock incrementality for workspace products.
//!
//! Salsa tracks per-file queries. This module tracks workspace-sized values
//! that must not enter the Salsa dependency graph — notably
//! [`hir_def::pathres::ResolutionContext`]. Once a per-file query reads
//! `unit_scope` / `design_map` / `unit_index` through Salsa, every file hangs
//! off the whole project.
//!
//! Two clocks:
//! - Salsa revision `r` — any input change
//! - Structure epoch `s` — a dirty file's declaration skeleton changed
//!
//! Three product kinds:
//! - **Structure products** (`ResolutionContext`, `SemanticSnapshotInputs`):
//!   keyed by `s`, memoized in `ProductCell` so a foreground request can
//!   preempt a background prewarm
//! - **File shards** (`FileSemanticIndex`, `FileModuleEdges`): keyed by
//!   `(generation, FileId)` against a single per-file generation clock
//! - **Merged indexes** (`ReferenceIndex`, `ModuleEdgeIndex`): folds over
//!   shards; a Drop epoch forces a full rebuild
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
