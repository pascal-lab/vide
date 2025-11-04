pub mod scope;

pub use scope::{
    BlockScopeCompletionExt, ModuleScopeCompletionExt, PackageScopeCompletionExt,
    SubroutineScopeCompletionExt, UnitScopeCompletionExt,
};

use crate::hir_def::Ident;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionEntry {
    pub name: Ident,
    pub kind: CompletionEntryKind,
    pub detail: Option<String>,
}

impl CompletionEntry {
    pub fn new(name: Ident, kind: CompletionEntryKind) -> Self {
        Self { name, kind, detail: None }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompletionEntryKind {
    Module,
    Port,
    Parameter,
    Variable,
    Net,
    Instance,
    Block,
    Function,
    Statement,
    Type,
    Import,
}

impl CompletionEntryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompletionEntryKind::Module => "module",
            CompletionEntryKind::Port => "port",
            CompletionEntryKind::Parameter => "parameter",
            CompletionEntryKind::Variable => "variable",
            CompletionEntryKind::Net => "net",
            CompletionEntryKind::Instance => "instance",
            CompletionEntryKind::Block => "block",
            CompletionEntryKind::Function => "function",
            CompletionEntryKind::Statement => "statement",
            CompletionEntryKind::Type => "type",
            CompletionEntryKind::Import => "import",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionScope {
    Local,
    Subroutine,
    Module,
    Package,
    File,
    Unit,
    Class,
}

#[derive(Debug, Clone)]
pub struct ScopedCompletionEntry {
    pub entry: CompletionEntry,
    pub scope: CompletionScope,
}

#[derive(Debug, Clone)]
pub struct DotField {
    pub name: Ident,
    pub detail: Option<String>,
    pub kind: DotFieldKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotFieldKind {
    Field,
    Method,
}
