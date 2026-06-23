#![feature(try_blocks)]
#![feature(decl_macro)]

pub use hir::base_db::Cancelled;
pub use index::SymbolKind;
pub use range::{ErasedFileAstId, FilePosition, FileRange, RangeInfo};
pub type Cancellable<T> = Result<T, Cancelled>;

pub mod analysis;
pub mod analysis_host;
pub mod definitions;
pub mod markup;
#[cfg(test)]
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
pub mod inlay_hint;
#[cfg(test)]
mod macro_hover_tests;
pub mod range;
pub mod references;
pub mod rename;
pub mod selection_ranges;
pub mod semantic_tokens;
pub mod signature_help;
pub(crate) mod source_targets;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod verilog_2005;
pub mod workspace_symbols;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeVisibility {
    Public,
    Private,
}
