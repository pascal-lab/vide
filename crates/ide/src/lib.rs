#![feature(try_blocks)]
#![feature(decl_macro)]

pub use base_db::Cancelled;
use hir::hir_def::{
    block::BlockId,
    expr::declarator::DeclId,
    module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
    opaque::OpaqueKind,
    stmt::StmtId,
};
use syntax::{SyntaxKind, ast, match_ast_kind};
pub type Cancellable<T> = Result<T, Cancelled>;

pub mod analysis;
pub mod analysis_host;
pub mod definitions;
pub mod markup;
pub mod navigation_target;
pub mod render;
pub mod source_change;

pub mod code_action;
pub mod code_lens;
pub mod completion;
pub mod diagnostics;
pub mod document_highlight;
pub mod document_symbols;
pub mod folding_ranges;
pub mod formatting;
pub mod goto_declaration;
pub mod goto_definition;
pub mod hover;
pub mod inlay_hint;
pub mod references;
pub mod rename;
pub mod selection_ranges;
pub mod semantic_tokens;
pub mod signature_help;
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod verilog_2005;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SymbolKind {
    Module,
    Config,
    NonAnsiPortLabel,
    PortDecl,
    ParamDecl,
    NetDecl,
    DataDecl,
    Genvar,
    Specparam,
    Typedef,
    Instance,
    Block,
    Stmt,
    Fn,
    Generate,
    Interface,
    Region,
    Opaque,
}

impl SymbolKind {
    pub fn from_syntax_kind(kind: SyntaxKind) -> Self {
        match_ast_kind! { kind,
            ast::ModuleDeclaration where kind == SyntaxKind::MODULE_DECLARATION => SymbolKind::Module,
            ast::ConfigDeclaration => SymbolKind::Config,
            ast::NonAnsiPort => SymbolKind::NonAnsiPortLabel,
            ast::PortDeclaration => SymbolKind::PortDecl,
            ast::ParameterDeclaration => SymbolKind::ParamDecl,
            ast::NetDeclaration => SymbolKind::NetDecl,
            ast::DataDeclaration => SymbolKind::DataDecl,
            ast::GenvarDeclaration => SymbolKind::Genvar,
            ast::SpecparamDeclaration => SymbolKind::Specparam,
            ast::TypedefDeclaration => SymbolKind::Typedef,
            ast::Declarator => SymbolKind::DataDecl,
            ast::HierarchicalInstance => SymbolKind::Instance,

            ast::BlockStatement => SymbolKind::Block,
            ast::Statement => SymbolKind::Stmt, // the order of these two is important

            ast::FunctionDeclaration => SymbolKind::Fn,
            _ => SymbolKind::Opaque,
        }
    }

    pub fn from_opaque_kind(kind: OpaqueKind, syntax_kind: SyntaxKind) -> Self {
        match kind {
            OpaqueKind::Generate => SymbolKind::Generate,
            OpaqueKind::Udp => SymbolKind::Module,
            OpaqueKind::Config => SymbolKind::Config,
            OpaqueKind::Library => SymbolKind::Opaque,
            OpaqueKind::Specparam => SymbolKind::Specparam,
            OpaqueKind::Genvar => SymbolKind::Genvar,
            OpaqueKind::Statement => SymbolKind::Stmt,
            OpaqueKind::BlockItem | OpaqueKind::FileItem | OpaqueKind::ModuleItem => {
                SymbolKind::from_syntax_kind(syntax_kind)
            }
            OpaqueKind::Specify | OpaqueKind::DefParam => SymbolKind::Opaque,
        }
    }
}

// TODO: const impl
impl From<ModuleId> for SymbolKind {
    fn from(_: ModuleId) -> Self {
        SymbolKind::Module
    }
}

impl From<BlockId> for SymbolKind {
    fn from(_: BlockId) -> Self {
        SymbolKind::Block
    }
}

impl From<NonAnsiPortId> for SymbolKind {
    fn from(_: NonAnsiPortId) -> Self {
        SymbolKind::NonAnsiPortLabel
    }
}

impl From<DeclId> for SymbolKind {
    fn from(_: DeclId) -> Self {
        SymbolKind::DataDecl
    }
}

impl From<InstanceId> for SymbolKind {
    fn from(_: InstanceId) -> Self {
        SymbolKind::Instance
    }
}

impl From<StmtId> for SymbolKind {
    fn from(_: StmtId) -> Self {
        SymbolKind::Stmt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeVisibility {
    Public,
    Private,
}
