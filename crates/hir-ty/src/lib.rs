//! Semantic types, inference, and type display.
//!
//! This crate interprets `hir-def` definitions and expressions as semantic
//! types. Definition-kind matching across this seam is exhaustive so adding a
//! new definition kind forces the type layer to classify it. This crate must
//! not depend on semantic adapters or IDE features.

pub mod db;
pub mod display;
pub mod type_infer;
