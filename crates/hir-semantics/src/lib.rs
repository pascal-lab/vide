//! Syntax-to-HIR semantic adapter.
//!
//! This crate maps syntax nodes and source ranges to the ECS-style identifiers
//! owned by `hir-def`, `hir-ty`, and `preproc-expand`. It is intentionally not
//! a stable, high-level HIR facade: IDE features currently consume those
//! implementation layers directly. A future facade would require
//! self-contained definition, scope, resolution, and type interfaces rather
//! than re-exporting their identifiers.
//!
//! Compiler implementation layers must not depend back on this adapter.

pub mod semantics;

#[cfg(test)]
mod preproc_integration_tests;
