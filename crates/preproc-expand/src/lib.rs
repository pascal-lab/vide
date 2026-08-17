//! Incremental compilation planning, preprocessing, and macro expansion.
//!
//! This crate owns `PreprocDb`, compilation plans, source-preprocessing
//! integration, macro files, expansion source maps, and the macro reference
//! index. It may depend on `base-db` and the pure `preproc` model, but must not
//! depend on lowered definitions or semantic types. It must not re-export
//! lower-layer crates as compatibility namespaces.

pub mod compilation_plan;
pub mod context;
pub mod db;
pub mod file;
pub mod macro_file;
pub mod preproc;
pub mod profile_compiler;
pub mod source_db;
