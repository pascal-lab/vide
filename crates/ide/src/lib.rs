#![feature(try_blocks)]
#![feature(decl_macro)]

pub use base_db::{
    Cancelled,
    analysis_snapshot::{AnalysisSnapshotId, CompilationContext},
};
pub use hir_def::symbol::DefKind;
pub use range::{ErasedFileAstId, FilePosition, FileRange, RangeInfo};
pub type Cancellable<T> = Result<T, Cancelled>;

pub mod analysis;
pub mod analysis_host;
pub mod definitions;
pub(crate) mod manifest;
pub mod markup;
pub(crate) mod module_resolution;
pub mod navigation_target;
pub mod render;
pub mod source_change;

pub mod code_action;
pub mod code_lens;
pub mod completion;
pub mod db;
pub mod diagnostics;
pub mod document_highlight;
pub mod document_symbols;
pub mod folding_ranges;
pub mod formatting;
pub mod goto_declaration;
pub mod goto_definition;
pub mod hover;
pub(crate) mod incrementality;
pub mod inlay_hint;
#[cfg(test)]
mod macro_hover_tests;
pub(crate) mod name_index;
pub mod range;
pub mod references;
pub mod rename;
pub mod selection_ranges;
pub mod semantic_index;
pub(crate) mod semantic_target;
pub mod semantic_tokens;
pub mod signature_help;
#[cfg(test)]
mod test_utils;
pub(crate) mod token;
#[cfg(test)]
mod verilog_2005;
pub mod workspace_symbols;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeVisibility {
    Public,
    Private,
}
