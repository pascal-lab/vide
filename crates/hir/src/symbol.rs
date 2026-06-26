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
    Block,
    GenerateBlock,
    Subroutine,
    SubroutinePort,
    NonAnsiPort,
    Typedef,
    Enum,
    Struct,
    Net,
    Variable,
    Param,
    Port,
    Genvar,
    Specparam,
    Instance,
    ClassField,
    Method,
    Modport,
    ClockingBlock,
    Sequence,
    Property,
    Stmt,
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
            DefKind::Block => SymbolKind::Block,
            DefKind::GenerateBlock => SymbolKind::Generate,
            DefKind::Subroutine | DefKind::Method => SymbolKind::Fn,
            DefKind::NonAnsiPort => SymbolKind::NonAnsiPortLabel,
            DefKind::SubroutinePort | DefKind::Port => SymbolKind::PortDecl,
            DefKind::Typedef | DefKind::Enum => SymbolKind::Typedef,
            DefKind::Struct => SymbolKind::Struct,
            DefKind::Net => SymbolKind::NetDecl,
            DefKind::Variable | DefKind::ClassField => SymbolKind::DataDecl,
            DefKind::Param => SymbolKind::ParamDecl,
            DefKind::Genvar => SymbolKind::Genvar,
            DefKind::Specparam => SymbolKind::Specparam,
            DefKind::Instance => SymbolKind::Instance,
            DefKind::Stmt => SymbolKind::Stmt,
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum NameResolution<T> {
    Unique(T),
    Ambiguous(SmallVec<[T; 2]>),
    Unresolved,
}

impl<T> NameResolution<T> {
    pub fn unique(self) -> Option<T> {
        match self {
            NameResolution::Unique(value) => Some(value),
            NameResolution::Ambiguous(_) | NameResolution::Unresolved => None,
        }
    }
}

impl NameScope {
    pub fn insert_type(&mut self, ident: &Ident, def_id: DefId) {
        Self::insert(&mut self.types, ident, def_id);
    }

    pub fn insert_type_opt(&mut self, ident: &Option<Ident>, def_id: DefId) {
        if let Some(ident) = ident {
            self.insert_type(ident, def_id);
        }
    }

    pub fn insert_value(&mut self, ident: &Ident, def_id: DefId) {
        Self::insert(&mut self.values, ident, def_id);
    }

    pub fn insert_value_opt(&mut self, ident: &Option<Ident>, def_id: DefId) {
        if let Some(ident) = ident {
            self.insert_value(ident, def_id);
        }
    }

    pub fn insert_assertion(&mut self, ident: &Ident, def_id: DefId) {
        Self::insert(&mut self.assertions, ident, def_id);
    }

    pub fn lookup_merged(&self, ident: &Ident) -> Option<SmallVec<[DefId; 1]>> {
        let mut defs = SmallVec::new();
        if let Some(type_defs) = self.types.get(ident) {
            defs.extend_from_slice(type_defs);
        }
        if let Some(value_defs) = self.values.get(ident) {
            defs.extend_from_slice(value_defs);
        }

        (!defs.is_empty()).then_some(defs)
    }

    pub fn iter_merged(&self) -> impl Iterator<Item = (&Ident, SmallVec<[DefId; 1]>)> + '_ {
        self.types
            .iter()
            .map(|(ident, type_defs)| {
                let mut defs = SmallVec::from_slice(type_defs);
                if let Some(value_defs) = self.values.get(ident) {
                    defs.extend_from_slice(value_defs);
                }
                (ident, defs)
            })
            .chain(
                self.values
                    .iter()
                    .filter(|(ident, _)| !self.types.contains_key(*ident))
                    .map(|(ident, defs)| (ident, SmallVec::from_slice(defs))),
            )
    }

    pub fn module_ids(
        &self,
        db: &dyn InternDb,
        ident: &Ident,
    ) -> NameResolution<crate::hir_def::module::ModuleId> {
        let entries = self
            .values
            .get(ident)
            .into_iter()
            .flat_map(|defs| defs.iter())
            .filter_map(|def_id| def_id.as_module(db))
            .collect::<SmallVec<[_; 2]>>();

        match entries.as_slice() {
            [module_id] => NameResolution::Unique(*module_id),
            [] => NameResolution::Unresolved,
            _ => NameResolution::Ambiguous(entries),
        }
    }

    pub fn module_names<'a>(
        &'a self,
        db: &'a dyn InternDb,
    ) -> impl Iterator<Item = &'a Ident> + 'a {
        self.values.iter().filter_map(move |(ident, defs)| {
            defs.iter().any(|def_id| def_id.as_module(db).is_some()).then_some(ident)
        })
    }

    pub fn typedef_names<'a>(
        &'a self,
        db: &'a dyn InternDb,
    ) -> impl Iterator<Item = &'a Ident> + 'a {
        self.types.iter().filter_map(move |(ident, defs)| {
            defs.iter().any(|def_id| matches!(def_id.loc(db), DefLoc::Typedef(_))).then_some(ident)
        })
    }

    fn insert(map: &mut FxHashMap<Ident, SmallVec<[DefId; 1]>>, ident: &Ident, def_id: DefId) {
        let defs = map.entry(ident.clone()).or_default();
        if !defs.contains(&def_id) {
            defs.push(def_id);
        }
    }
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
