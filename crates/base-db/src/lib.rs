//! Incremental compiler inputs, project state, and shared database primitives.
//!
//! This crate owns source text, source roots, project configuration, and
//! shared database primitives. The lowest database layer must not depend on
//! preprocessing, lowered definitions, name resolution, or semantic types.

pub use salsa::{self, Cancelled};

pub mod analysis_snapshot;
pub mod change;
pub mod diagnostics_config;
pub mod project;
pub mod source_db;
pub mod source_root;
