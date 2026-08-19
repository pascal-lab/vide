//! Overlay and parse-dependency book-keeping for workspace products.
//!
//! Salsa tracks per-file queries and the L0 source catalog
//! (`source_unit_catalog`). This module stores values that are not salsa
//! inputs: fingerprint-keyed generated units, and the include edges of a
//! paid parse. Once a per-file query reads `unit_scope` through Salsa, every
//! file hangs off the whole project; resolution is therefore derived from
//! the current catalog on each request, not stored as a salsa query.
//!
//! Production catalog:
//! `source_unit_catalog(db).with_overlay(store.generated_units())`.
//! Generated units are stored under
//! `(FileId, compilation_unit_snapshot.fingerprint)` so a later snapshot
//! cannot observe a previous artifact's names. Making them a salsa query
//! over `compilation_unit_artifact` would force a paid parse of every
//! previously-parsed CU on the next fold (T1).
//!
//! `file_decls` is unbounded; `file_facts` keeps the parse LRU. Sharing
//! that LRU made a 1280-file 2000-wire `file_decls` refetch after one
//! edit cost 379ms. With decls unbounded it is 0.17ms
//! (`design_graph_refold_after_body_edit`).
//!
//! New caches belong in Salsa (per-file, dependency-tracked) or in
//! [`ProductStore`] (overlay and parse-deps). A third cache in a feature
//! function or on `RootDb` is a bug.

mod store;

pub(crate) use store::ProductStore;
