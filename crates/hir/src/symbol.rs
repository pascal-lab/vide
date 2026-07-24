use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use utils::impl_from;

use crate::{
    base_db::salsa,
    container::{InContainer, InFile, InModule, InSubroutine},
    db::{HirDb, InternDb},
    def_id::DefId,
    hir_def::{
        Ident,
        block::BlockId,
        checker::{CheckerId, CheckerPortId},
        covergroup::{CovergroupId, CoverpointId, CrossId},
        expr::declarator::DeclId,
        file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
        module::{
            ModuleId,
            clocking::{ClockingBlockId, ClockingSignalId},
            generate::GenerateBlockId,
            instantiation::InstanceId,
            modport::ModportId,
            port::NonAnsiPortId,
        },
        stmt::StmtId,
        subroutine::{LocalSubroutineId, SubroutinePortId},
        typedef::TypedefId,
    },
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct DefOrigin(pub salsa::InternId);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DefOriginLoc {
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
    Subroutine(InContainer<LocalSubroutineId>),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Modport(InModule<ModportId>),
    ClockingBlock(InModule<ClockingBlockId>),
    ClockingSignal(InContainer<ClockingSignalId>),
    Checker(InContainer<CheckerId>),
    CheckerPort(InContainer<CheckerPortId>),
    Covergroup(InContainer<CovergroupId>),
    Coverpoint(InContainer<CoverpointId>),
    Cross(InContainer<CrossId>),
    Stmt(InContainer<StmtId>),
}

impl_from! { DefOriginLoc =>
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
    Subroutine(InContainer<LocalSubroutineId>),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Modport(InModule<ModportId>),
    ClockingBlock(InModule<ClockingBlockId>),
    ClockingSignal(InContainer<ClockingSignalId>),
    Checker(InContainer<CheckerId>),
    CheckerPort(InContainer<CheckerPortId>),
    Covergroup(InContainer<CovergroupId>),
    Coverpoint(InContainer<CoverpointId>),
    Cross(InContainer<CrossId>),
    Stmt(InContainer<StmtId>),
}

impl DefOrigin {
    pub fn new(db: &dyn InternDb, loc: impl Into<DefOriginLoc>) -> Self {
        db.intern_def_origin(loc.into())
    }

    pub fn loc(self, db: &dyn InternDb) -> DefOriginLoc {
        db.lookup_intern_def_origin(self)
    }

    pub fn as_module(self, db: &dyn InternDb) -> Option<ModuleId> {
        match self.loc(db) {
            DefOriginLoc::Module(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_config(self, db: &dyn InternDb) -> Option<InFile<ConfigDeclId>> {
        match self.loc(db) {
            DefOriginLoc::Config(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_library(self, db: &dyn InternDb) -> Option<InFile<LibraryDeclId>> {
        match self.loc(db) {
            DefOriginLoc::Library(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_udp(self, db: &dyn InternDb) -> Option<InFile<UdpDeclId>> {
        match self.loc(db) {
            DefOriginLoc::Udp(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_block(self, db: &dyn InternDb) -> Option<BlockId> {
        match self.loc(db) {
            DefOriginLoc::Block(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_generate_block(self, db: &dyn InternDb) -> Option<GenerateBlockId> {
        match self.loc(db) {
            DefOriginLoc::GenerateBlock(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_subroutine(self, db: &dyn InternDb) -> Option<InContainer<LocalSubroutineId>> {
        match self.loc(db) {
            DefOriginLoc::Subroutine(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_subroutine_port(self, db: &dyn InternDb) -> Option<InSubroutine<SubroutinePortId>> {
        match self.loc(db) {
            DefOriginLoc::SubroutinePort(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_non_ansi_port(self, db: &dyn InternDb) -> Option<InModule<NonAnsiPortId>> {
        match self.loc(db) {
            DefOriginLoc::NonAnsiPort(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_decl(self, db: &dyn InternDb) -> Option<InContainer<DeclId>> {
        match self.loc(db) {
            DefOriginLoc::Decl(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_typedef(self, db: &dyn InternDb) -> Option<InContainer<TypedefId>> {
        match self.loc(db) {
            DefOriginLoc::Typedef(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_instance(self, db: &dyn InternDb) -> Option<InModule<InstanceId>> {
        match self.loc(db) {
            DefOriginLoc::Instance(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_modport(self, db: &dyn InternDb) -> Option<InModule<ModportId>> {
        match self.loc(db) {
            DefOriginLoc::Modport(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_clocking_block(self, db: &dyn InternDb) -> Option<InModule<ClockingBlockId>> {
        match self.loc(db) {
            DefOriginLoc::ClockingBlock(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_clocking_signal(self, db: &dyn InternDb) -> Option<InContainer<ClockingSignalId>> {
        match self.loc(db) {
            DefOriginLoc::ClockingSignal(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_checker(self, db: &dyn InternDb) -> Option<InContainer<CheckerId>> {
        match self.loc(db) {
            DefOriginLoc::Checker(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_checker_port(self, db: &dyn InternDb) -> Option<InContainer<CheckerPortId>> {
        match self.loc(db) {
            DefOriginLoc::CheckerPort(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_covergroup(self, db: &dyn InternDb) -> Option<InContainer<CovergroupId>> {
        match self.loc(db) {
            DefOriginLoc::Covergroup(id) => Some(id),
            _ => None,
        }
    }

    pub fn as_stmt(self, db: &dyn InternDb) -> Option<InContainer<StmtId>> {
        match self.loc(db) {
            DefOriginLoc::Stmt(id) => Some(id),
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
    Udp,
    Config,
    Library,
    Block,
    GenerateBlock,
    Subroutine,
    SubroutinePort,
    NonAnsiPort,
    Typedef,
    Net,
    Variable,
    Param,
    Port,
    Genvar,
    Specparam,
    Instance,
    Modport,
    ClockingBlock,
    ClockingSignal,
    Checker,
    CheckerPort,
    Covergroup,
    Coverpoint,
    Cross,
    Stmt,
}

impl DefKind {
    pub fn is_instantiable_def(self) -> bool {
        matches!(
            self,
            DefKind::Module
                | DefKind::Interface
                | DefKind::Program
                | DefKind::Checker
                | DefKind::Covergroup
        )
    }

    pub fn symbol_kind(self) -> SymbolKind {
        match self {
            DefKind::Module => SymbolKind::Module,
            DefKind::Interface => SymbolKind::Interface,
            DefKind::Package | DefKind::Program => SymbolKind::Unknown,
            DefKind::Udp => SymbolKind::Primitive,
            DefKind::Config => SymbolKind::Config,
            DefKind::Library => SymbolKind::Library,
            DefKind::Block => SymbolKind::Block,
            DefKind::GenerateBlock => SymbolKind::Generate,
            DefKind::Subroutine => SymbolKind::Fn,
            DefKind::NonAnsiPort => SymbolKind::NonAnsiPortLabel,
            DefKind::SubroutinePort | DefKind::Port => SymbolKind::PortDecl,
            DefKind::CheckerPort => SymbolKind::PortDecl,
            DefKind::Typedef => SymbolKind::Typedef,
            DefKind::Net => SymbolKind::NetDecl,
            DefKind::Variable => SymbolKind::DataDecl,
            DefKind::Param => SymbolKind::ParamDecl,
            DefKind::Genvar => SymbolKind::Genvar,
            DefKind::Specparam => SymbolKind::Specparam,
            DefKind::Instance => SymbolKind::Instance,
            DefKind::Modport
            | DefKind::ClockingBlock
            | DefKind::ClockingSignal
            | DefKind::Checker
            | DefKind::Covergroup
            | DefKind::Coverpoint
            | DefKind::Cross => SymbolKind::Unknown,
            DefKind::Stmt => SymbolKind::Stmt,
        }
    }

    pub fn name_context(self) -> NameContext {
        match self {
            DefKind::Module
            | DefKind::Interface
            | DefKind::Package
            | DefKind::Program
            | DefKind::Checker
            | DefKind::Covergroup
            | DefKind::Typedef => NameContext::Type,
            _ => NameContext::Value,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Import {
    pub package: Ident,
    pub name: Option<Ident>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameContext {
    Type,
    Value,
    Listing,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Resolution<T> {
    Unresolved,
    Unique(T),
    Ambiguous(SmallVec<[T; 2]>),
}

impl<T> Resolution<T> {
    pub fn candidates(&self) -> &[T] {
        match self {
            Resolution::Unresolved => &[],
            Resolution::Unique(value) => std::slice::from_ref(value),
            Resolution::Ambiguous(candidates) => candidates,
        }
    }

    pub fn into_candidates(self) -> SmallVec<[T; 2]> {
        match self {
            Resolution::Unresolved => SmallVec::new(),
            Resolution::Unique(value) => {
                let mut candidates = SmallVec::new();
                candidates.push(value);
                candidates
            }
            Resolution::Ambiguous(candidates) => candidates,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.candidates().iter()
    }

    pub fn is_unresolved(&self) -> bool {
        matches!(self, Resolution::Unresolved)
    }

    pub fn or_else(self, fallback: impl FnOnce() -> Self) -> Self {
        if self.is_unresolved() { fallback() } else { self }
    }

    pub fn into_option(self) -> Option<Self> {
        if self.is_unresolved() { None } else { Some(self) }
    }

    pub fn map<U>(self, mut map: impl FnMut(T) -> U) -> Resolution<U> {
        match self {
            Resolution::Unresolved => Resolution::Unresolved,
            Resolution::Unique(value) => Resolution::Unique(map(value)),
            Resolution::Ambiguous(candidates) => {
                Resolution::Ambiguous(candidates.into_iter().map(map).collect())
            }
        }
    }
}

impl<T: Copy> Resolution<T> {
    pub fn unique(&self) -> Option<T> {
        match self {
            Resolution::Unique(value) => Some(*value),
            Resolution::Ambiguous(_) | Resolution::Unresolved => None,
        }
    }
}

impl<T> From<T> for Resolution<T> {
    fn from(value: T) -> Self {
        Resolution::Unique(value)
    }
}

impl<T: Eq> Resolution<T> {
    pub fn from_candidates(candidates: impl IntoIterator<Item = T>) -> Self {
        let mut unique = SmallVec::<[T; 2]>::new();
        for candidate in candidates {
            if !unique.contains(&candidate) {
                unique.push(candidate);
            }
        }

        match unique.len() {
            0 => Resolution::Unresolved,
            1 => Resolution::Unique(unique.pop().expect("candidate length was checked")),
            _ => Resolution::Ambiguous(unique),
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

    pub fn lookup(&self, ctx: NameContext, ident: &Ident) -> Resolution<DefId> {
        let candidates = match ctx {
            NameContext::Type => self.types.get(ident).map(SmallVec::as_slice).unwrap_or_default(),
            NameContext::Value => {
                self.values.get(ident).map(SmallVec::as_slice).unwrap_or_default()
            }
            NameContext::Listing => return Resolution::from_candidates(self.lookup_listing(ident)),
        };
        Resolution::from_candidates(candidates.iter().copied())
    }

    pub fn lookup_listing(&self, ident: &Ident) -> SmallVec<[DefId; 1]> {
        let mut defs = SmallVec::new();
        if let Some(type_defs) = self.types.get(ident) {
            defs.extend_from_slice(type_defs);
        }
        if let Some(value_defs) = self.values.get(ident) {
            defs.extend_from_slice(value_defs);
        }
        defs
    }

    pub fn iter_listing(&self) -> impl Iterator<Item = (&Ident, SmallVec<[DefId; 1]>)> + '_ {
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
        db: &dyn HirDb,
        ident: &Ident,
    ) -> Resolution<crate::hir_def::module::ModuleId> {
        let entries = self
            .types
            .get(ident)
            .into_iter()
            .flat_map(|defs| defs.iter())
            .filter(|def_id| def_id.kind(db).is_instantiable_def())
            .filter_map(|def_id| def_id.as_module(db))
            .collect::<SmallVec<[_; 2]>>();
        Resolution::from_candidates(entries)
    }

    pub fn package_ids(
        &self,
        db: &dyn HirDb,
        ident: &Ident,
    ) -> Resolution<crate::hir_def::module::PackageId> {
        let entries = self
            .types
            .get(ident)
            .into_iter()
            .flat_map(|defs| defs.iter())
            .filter(|def_id| def_id.kind(db) == DefKind::Package)
            .filter_map(|def_id| def_id.as_module(db))
            .collect::<SmallVec<[_; 2]>>();
        Resolution::from_candidates(entries)
    }

    pub fn module_names<'a>(&'a self, db: &'a dyn HirDb) -> impl Iterator<Item = &'a Ident> + 'a {
        self.types.iter().filter_map(move |(ident, defs)| {
            defs.iter()
                .any(|def_id| {
                    def_id.kind(db).is_instantiable_def() && def_id.as_module(db).is_some()
                })
                .then_some(ident)
        })
    }

    pub fn typedef_names<'a>(&'a self, db: &'a dyn HirDb) -> impl Iterator<Item = &'a Ident> + 'a {
        self.types.iter().filter_map(move |(ident, defs)| {
            defs.iter()
                .any(|def_id| matches!(def_id.primary_origin(db).loc(db), DefOriginLoc::Typedef(_)))
                .then_some(ident)
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
