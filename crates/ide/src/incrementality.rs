//! Parse-dependency book-keeping for workspace products.
//!
//! Salsa tracks per-file queries and the L0 source catalog
//! (`source_unit_catalog`). This module stores values that are not salsa
//! inputs: the include edges of a paid parse. Those files are the locator
//! for macro-generated owners (`HirFileId::Macro`). Resolution does not
//! merge generated names into the catalog.
//!
//! Once a per-file query reads `unit_scope` through Salsa, every file hangs
//! off the whole project; resolution is therefore derived from the current
//! locator on each request, not stored as a salsa query.
//!
//! `file_decls` is unbounded; `file_facts` keeps the parse LRU. Sharing
//! that LRU made a 1280-file 2000-wire `file_decls` refetch after one
//! edit cost 379ms. With decls unbounded it is 0.17ms
//! (`design_graph_refold_after_body_edit`).
//!
//! New caches belong in Salsa (per-file, dependency-tracked) or in
//! [`ProductStore`] (parse-deps). A third cache in a feature function or
//! on `RootDb` is a bug.

mod store;

pub(crate) use store::ProductStore;
