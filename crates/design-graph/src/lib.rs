//! Compilation-unit design-unit facts.
//!
//! This crate owns unexpanded per-file extract and the name-join types. It
//! does not depend on `hir-def` or `ide`. Graph fold is a pure function of
//! salsa `file_facts` plus an optional generated-unit map supplied by the
//! caller.

pub mod db;
pub mod facts;
pub mod graph;
pub mod hit;
pub mod unit;

pub use db::{DesignGraphDb, set_file_facts_lru_capacity};
pub use facts::{FileFacts, ImportSpec, InstantiationSite, Mention, PackageRefSite};
pub use graph::{DesignGraph, GeneratedFileUnits, GeneratedUnits, GraphResolution, UnitMeta};
pub use hit::{CursorHit, hit_at};
pub use unit::{InstantiationRole, UnitId, UnitKind, UnitNode, UnitOrigin};
