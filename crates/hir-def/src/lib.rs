//! Lowered definitions and name resolution.
//!
//! This crate is the ECS-style definition implementation: arenas, local
//! indexes, container IDs, scopes, source maps, and resolution results live
//! here. It may depend on `preproc-expand`, but must not depend on type
//! inference, semantic adapters, or IDE features. Its identifiers are an
//! explicit workspace-internal interface, not a stable object-oriented
//! facade.

#![feature(decl_macro)]

pub mod container;
pub mod db;
pub mod def_id;
pub mod has_source;
pub mod hir_def;
pub mod pathres;
pub mod region_tree;
pub mod scope;
pub mod source_map;
pub mod symbol;
