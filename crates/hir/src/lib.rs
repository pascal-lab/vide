//! High-level HIR facade for IDE consumers.
//!
//! This crate is the *only* HIR entry point IDE code should depend on. It
//! exposes a small, object-oriented surface -- `Semantics`, `Definition`,
//! `Module`, `Type`, `Source`, `File`, `ParsedFile` -- over the arena/ECS
//! implementation crates (`hir-def`, `hir-ty`, `hir-semantics`,
//! `preproc-expand`).
//!
//! Implementation identifiers (`DefId`, `LocalModuleId`, `DefLoc`, raw source
//! maps, `InternDb`, ...) stay crate-private here. Migrating IDE to this
//! facade keeps HIR representation changes from rippling across dozens of IDE
//! files; the compiler layers must not depend back on this crate.

mod file;
mod module;
mod parsed_file;
mod semantics;

pub use file::File;
pub use module::{Module, Source};
pub use parsed_file::ParsedFile;
pub use semantics::{Definition, Semantics};

/// Re-exported verbatim: `hir-ty`'s `Type` is already a high-level handle
/// (an `Arc` over the inferred result) and carries no arena/local identifiers
/// in its public surface, so it is part of the facade without a wrapper.
pub use hir_ty::Type;