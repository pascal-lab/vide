// migration_only: RFC 0001 Phase 2 moved the directive index boundary to
// `preproc`. Keep this re-export only until existing base_db callers are
// switched to import the preproc crate directly.
pub use preproc::directive_index::*;
