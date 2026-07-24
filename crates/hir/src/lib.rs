#![feature(decl_macro)]

pub use base_db::{self, impl_intern_key, impl_intern_lookup};
pub use preproc_expand::{self, compilation_plan, file, macro_file, preproc};
pub mod container;
pub mod db;
pub mod def_id;
pub mod display;
pub mod has_source;
pub mod hir_def;
pub mod pathres;
pub mod region_tree;
pub mod scope;
pub mod semantics;
pub mod source_map;
pub mod symbol;
mod ty_db;
pub mod type_infer;

#[cfg(test)]
mod preproc_integration_tests;
