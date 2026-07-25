//! Incremental compiler inputs and project state.
//!
//! This crate owns source text, source roots, project configuration, change
//! application, and snapshot metadata. It is the lowest database layer and
//! must not depend on preprocessing, lowered definitions, name resolution, or
//! semantic types. Types owned by higher layers must not appear in its
//! interface.

pub use salsa::{self, Cancelled};

pub mod analysis_snapshot;
pub mod change;
pub mod diagnostics_config;
pub mod intern;
pub mod project;
pub mod source_db;
pub mod source_root;
