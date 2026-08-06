#![feature(try_blocks)]
#![feature(decl_macro)]

pub use base_db::{
    Cancelled,
    analysis_snapshot::{AnalysisSnapshotId, CompilationContext},
};
pub use range::{ErasedFileAstId, FilePosition, FileRange, RangeInfo};
use syntax::{SyntaxKind, ast, match_ast_kind};
pub type Cancellable<T> = Result<T, Cancelled>;

pub mod analysis;
pub mod analysis_host;
pub mod definitions;
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
#[cfg(test)]
mod index_benchmarks;
pub mod inlay_hint;
#[cfg(test)]
mod macro_hover_tests;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    Module,
    Config,
    Primitive,
    NonAnsiPortLabel,
    PortDecl,
    ParamDecl,
    NetDecl,
    DataDecl,
    Genvar,
    Specparam,
    Typedef,
    Struct,
    Instance,
    Block,
    Stmt,
    Fn,
    Generate,
    Specify,
    Interface,
    Library,
    Region,
    Unknown,
}

impl SymbolKind {
    pub fn from_syntax_kind(kind: SyntaxKind) -> Self {
        match_ast_kind! { kind,
            ast::ModuleDeclaration where kind == SyntaxKind::MODULE_DECLARATION => SymbolKind::Module,
            ast::ConfigDeclaration => SymbolKind::Config,
            ast::UdpDeclaration => SymbolKind::Primitive,
            ast::NonAnsiPort => SymbolKind::NonAnsiPortLabel,
            ast::PortDeclaration => SymbolKind::PortDecl,
            ast::ParameterDeclaration => SymbolKind::ParamDecl,
            ast::NetDeclaration => SymbolKind::NetDecl,
            ast::DataDeclaration => SymbolKind::DataDecl,
            ast::GenvarDeclaration => SymbolKind::Genvar,
            ast::LibraryDeclaration => SymbolKind::Library,
            ast::SpecparamDeclaration => SymbolKind::Specparam,
            ast::TypedefDeclaration => SymbolKind::Typedef,
            ast::Declarator => SymbolKind::DataDecl,
            ast::HierarchicalInstance => SymbolKind::Instance,

            ast::BlockStatement => SymbolKind::Block,
            ast::Statement => SymbolKind::Stmt, // the order of these two is important

            ast::FunctionDeclaration => SymbolKind::Fn,
            ast::SpecifyBlock => SymbolKind::Specify,
            _ => SymbolKind::Unknown,
        }
    }
}

impl From<hir_def::symbol::SymbolKind> for SymbolKind {
    fn from(kind: hir_def::symbol::SymbolKind) -> Self {
        match kind {
            hir_def::symbol::SymbolKind::Module => SymbolKind::Module,
            hir_def::symbol::SymbolKind::Config => SymbolKind::Config,
            hir_def::symbol::SymbolKind::Primitive => SymbolKind::Primitive,
            hir_def::symbol::SymbolKind::NonAnsiPortLabel => SymbolKind::NonAnsiPortLabel,
            hir_def::symbol::SymbolKind::PortDecl => SymbolKind::PortDecl,
            hir_def::symbol::SymbolKind::ParamDecl => SymbolKind::ParamDecl,
            hir_def::symbol::SymbolKind::NetDecl => SymbolKind::NetDecl,
            hir_def::symbol::SymbolKind::DataDecl => SymbolKind::DataDecl,
            hir_def::symbol::SymbolKind::Genvar => SymbolKind::Genvar,
            hir_def::symbol::SymbolKind::Specparam => SymbolKind::Specparam,
            hir_def::symbol::SymbolKind::Typedef => SymbolKind::Typedef,
            hir_def::symbol::SymbolKind::Struct => SymbolKind::Struct,
            hir_def::symbol::SymbolKind::Instance => SymbolKind::Instance,
            hir_def::symbol::SymbolKind::Block => SymbolKind::Block,
            hir_def::symbol::SymbolKind::Stmt => SymbolKind::Stmt,
            hir_def::symbol::SymbolKind::Fn => SymbolKind::Fn,
            hir_def::symbol::SymbolKind::Generate => SymbolKind::Generate,
            hir_def::symbol::SymbolKind::Specify => SymbolKind::Specify,
            hir_def::symbol::SymbolKind::Interface => SymbolKind::Interface,
            hir_def::symbol::SymbolKind::Library => SymbolKind::Library,
            hir_def::symbol::SymbolKind::Region => SymbolKind::Region,
            hir_def::symbol::SymbolKind::Unknown => SymbolKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeVisibility {
    Public,
    Private,
}
