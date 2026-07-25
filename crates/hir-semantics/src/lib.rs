//! Syntax-to-HIR semantic adapter.
//!
//! This crate maps syntax nodes and source ranges to the definition and
//! container identifiers owned by `hir-def`, using source-file and expansion
//! identifiers from `preproc-expand`. It intentionally does not depend on
//! `hir-ty`; type-aware IDE features compose this adapter with the type layer.
//!
//! This is not a stable, high-level HIR facade. A future facade would require
//! self-contained definition, scope, resolution, and type interfaces rather
//! than re-exporting implementation identifiers.
//!
//! Compiler implementation layers must not depend back on this adapter.

pub mod semantics;

#[cfg(test)]
mod preproc_integration_tests;
