#![feature(let_chains)]
#![feature(trait_alias)]
#![feature(decl_macro)]

pub mod completion;
pub mod container;
pub mod db;
pub mod display;
pub mod file;
pub mod has_source;
pub mod hir_def;
pub mod region_tree;
pub mod scope;
pub mod semantics;
pub mod source_map;

pub use completion::{
    CompletionEntry, CompletionEntryKind, CompletionScope, DotField, DotFieldKind,
    ScopedCompletionEntry,
};
pub use scope::{
    AnsiPortEntry, BlockEntry, BlockScope, ModuleEntry, ModuleScope, NonAnsiPortEntry, UnitEntry,
    UnitScope,
};
