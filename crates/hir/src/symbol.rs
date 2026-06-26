use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use utils::impl_from;

use crate::{
    base_db::salsa,
    container::{InContainer, InFile, InModule, InSubroutine},
    db::InternDb,
    hir_def::{
        Ident,
        block::BlockId,
        expr::declarator::DeclId,
        file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
        module::{
            ModuleId, generate::GenerateBlockId, instantiation::InstanceId, port::NonAnsiPortId,
        },
        stmt::StmtId,
        subroutine::{SubroutineId, SubroutinePortId},
        typedef::TypedefId,
    },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct DefId(pub salsa::InternId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefLoc {
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
    Subroutine(SubroutineId),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl_from! { DefLoc =>
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
    Subroutine(SubroutineId),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl DefId {
    pub fn new(db: &dyn InternDb, loc: impl Into<DefLoc>) -> Self {
        db.intern_def(loc.into())
    }

    pub fn loc(self, db: &dyn InternDb) -> DefLoc {
        db.lookup_intern_def(self)
    }

    pub fn as_module(self, db: &dyn InternDb) -> Option<ModuleId> {
        match self.loc(db) {
            DefLoc::Module(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_config(self, db: &dyn InternDb) -> Option<InFile<ConfigDeclId>> {
        match self.loc(db) {
            DefLoc::Config(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_library(self, db: &dyn InternDb) -> Option<InFile<LibraryDeclId>> {
        match self.loc(db) {
            DefLoc::Library(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_udp(self, db: &dyn InternDb) -> Option<InFile<UdpDeclId>> {
        match self.loc(db) {
            DefLoc::Udp(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_block(self, db: &dyn InternDb) -> Option<BlockId> {
        match self.loc(db) {
            DefLoc::Block(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_generate_block(self, db: &dyn InternDb) -> Option<GenerateBlockId> {
        match self.loc(db) {
            DefLoc::GenerateBlock(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_subroutine(self, db: &dyn InternDb) -> Option<SubroutineId> {
        match self.loc(db) {
            DefLoc::Subroutine(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_subroutine_port(self, db: &dyn InternDb) -> Option<InSubroutine<SubroutinePortId>> {
        match self.loc(db) {
            DefLoc::SubroutinePort(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_non_ansi_port(self, db: &dyn InternDb) -> Option<InModule<NonAnsiPortId>> {
        match self.loc(db) {
            DefLoc::NonAnsiPort(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_decl(self, db: &dyn InternDb) -> Option<InContainer<DeclId>> {
        match self.loc(db) {
            DefLoc::Decl(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_typedef(self, db: &dyn InternDb) -> Option<InContainer<TypedefId>> {
        match self.loc(db) {
            DefLoc::Typedef(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_instance(self, db: &dyn InternDb) -> Option<InModule<InstanceId>> {
        match self.loc(db) {
            DefLoc::Instance(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_stmt(self, db: &dyn InternDb) -> Option<InContainer<StmtId>> {
        match self.loc(db) {
            DefLoc::Stmt(id) => Some(id),
            _ => None,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefKind {
    Module,
    Interface,
    Package,
    Program,
    Class,
    Covergroup,
    Checker,
    Udp,
    Config,
    Library,
    Subroutine,
    SubroutinePort,
    Typedef,
    Enum,
    Struct,
    Net,
    Variable,
    Param,
    Port,
    Instance,
    ClassField,
    Method,
    Modport,
    ClockingBlock,
    Sequence,
    Property,
}

impl DefKind {
    pub fn symbol_kind(self) -> SymbolKind {
        match self {
            DefKind::Module => SymbolKind::Module,
            DefKind::Interface => SymbolKind::Interface,
            DefKind::Package
            | DefKind::Program
            | DefKind::Class
            | DefKind::Covergroup
            | DefKind::Checker
            | DefKind::Modport
            | DefKind::ClockingBlock
            | DefKind::Sequence
            | DefKind::Property => SymbolKind::Unknown,
            DefKind::Udp => SymbolKind::Primitive,
            DefKind::Config => SymbolKind::Config,
            DefKind::Library => SymbolKind::Library,
            DefKind::Subroutine | DefKind::Method => SymbolKind::Fn,
            DefKind::SubroutinePort | DefKind::Port => SymbolKind::PortDecl,
            DefKind::Typedef | DefKind::Enum => SymbolKind::Typedef,
            DefKind::Struct => SymbolKind::Struct,
            DefKind::Net => SymbolKind::NetDecl,
            DefKind::Variable | DefKind::ClassField => SymbolKind::DataDecl,
            DefKind::Param => SymbolKind::ParamDecl,
            DefKind::Instance => SymbolKind::Instance,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeKind {
    File,
    Package,
    Module,
    Interface,
    Program,
    Class,
    GenerateBlock,
    Block,
    Subroutine,
    Covergroup,
    ClockingBlock,
    Checker,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameScope {
    pub types: FxHashMap<Ident, SmallVec<[DefId; 1]>>,
    pub values: FxHashMap<Ident, SmallVec<[DefId; 1]>>,
    pub assertions: FxHashMap<Ident, SmallVec<[DefId; 1]>>,
    pub imports: SmallVec<[Import; 2]>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Import {
    pub named: Option<DefId>,
    pub wildcard_pkg: Option<DefId>,
}

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
