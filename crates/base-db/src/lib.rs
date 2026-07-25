//! Incremental compiler inputs, project state, and shared database primitives.
//!
//! This crate owns source text, source roots, project configuration, change
//! application, snapshot metadata, and the generic `Intern`/`Lookup` traits
//! and Salsa interning macros shared by higher query layers. The interning
//! substrate lives here because this is their lowest common dependency; it
//! does not own any layer-specific interned data.
//!
//! As the lowest database layer, this crate must not depend on preprocessing,
//! lowered definitions, name resolution, or semantic types. Types owned by
//! higher layers must not appear in its interface.

pub use salsa::{self, Cancelled};

pub mod analysis_snapshot;
pub mod change;
pub mod diagnostics_config;
pub mod intern;
pub mod project;
pub mod source_db;
pub mod source_root;
