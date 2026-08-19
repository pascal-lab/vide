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
//! T10 remeasured a salsa `source_unit_catalog` over position-free `file_decls`
//! after `Change::apply` stopped rewriting `file_kind` on every Modify.
//! After a body-only edit the decls are value-equal and salsa backdates
//! (`file_decls_backdate_across_a_body_only_edit`, first=1 after=1). The
//! earlier "salsa still re-executes" reading was that extra input write.
//!
//! What that measurement supports: the source catalog can live in salsa.
//! Generated units are a fingerprint-keyed overlay, not a salsa input
//! (`generated_overlay_is_outside_the_salsa_source_catalog`), so the
//! production catalog is a read-time merge:
//! `source_unit_catalog(db).with_overlay(generated)`. Making generated
//! units a salsa query over `compilation_unit_artifact` would force a paid
//! parse of every previously-parsed CU on the next fold (T1).
//!
//! Epoch, ProductCell preemption, and `ProductStore::fork` are leftovers of
//! the handwritten source-catalog clock. T14 deletes them. Overlay merge
//! does not need a generation counter. ProductCell stays until that
//! close-out so request-path behavior does not change in this step.
//!
//! `file_decls` is unbounded; `file_facts` keeps the parse LRU. Sharing
//! that LRU made a 1280-file 2000-wire `file_decls` refetch after one
//! edit cost 379ms. With decls unbounded it is 0.17ms
//! (`design_graph_refold_after_body_edit`).
//!
//! A generated-unit set change patches the graph for that file via
//! [`ProductStore::patch_design_graph`]. Generated units are stored under
//! `(FileId, compilation_unit_snapshot.fingerprint)` so a later snapshot
//! cannot observe a previous artifact's names.
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
