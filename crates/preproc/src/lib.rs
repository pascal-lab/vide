//! Database-independent source preprocessing model.
//!
//! This crate owns preprocessing events, trace identifiers, macro records, and
//! source ranges. It is pure model code: database integration, VFS mapping,
//! compilation planning, and macro-file expansion belong to `preproc-expand`.

pub mod source;
