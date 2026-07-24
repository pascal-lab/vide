pub use ::hir_def::{
    container, def_id, has_source, hir_def, pathres, region_tree, scope, source_map, symbol,
};
pub use base_db::{self, impl_intern_key, impl_intern_lookup};
pub use hir_ty::{display, type_infer};
pub use preproc_expand::{self, compilation_plan, file, macro_file, preproc};
pub mod db;
pub mod semantics;

#[cfg(test)]
mod preproc_integration_tests;
