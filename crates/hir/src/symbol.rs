use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use utils::impl_from;

use crate::{
    base_db::salsa,
    container::{
        InContainer, InFile, InFileOrModule, InModule, InScope, InSubroutine, SubroutineScope,
    },
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
        subroutine::SubroutinePortId,
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
    Subroutine(SubroutineScope),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Modport(InModule<ModportId>),
    ClockingBlock(InModule<ClockingBlockId>),
    ClockingSignal(InScope<ClockingSignalId>),
    Checker(InFileOrModule<CheckerId>),
    CheckerPort(InScope<CheckerPortId>),
    Covergroup(InFileOrModule<CovergroupId>),
    Coverpoint(InScope<CoverpointId>),
    Cross(InScope<CrossId>),
    Stmt(InContainer<StmtId>),
}

impl_from! { DefOriginLoc =>
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
    Subroutine(SubroutineScope),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Modport(InModule<ModportId>),
    ClockingBlock(InModule<ClockingBlockId>),
    ClockingSignal(InScope<ClockingSignalId>),
    Checker(InFileOrModule<CheckerId>),
    CheckerPort(InScope<CheckerPortId>),
    Covergroup(InFileOrModule<CovergroupId>),
    Coverpoint(InScope<CoverpointId>),
    Cross(InScope<CrossId>),
    Stmt(InContainer<StmtId>),
}

macro_rules! impl_origin_cast {
    ($method:ident, $variant:ident, $ty:ty) => {
        pub fn $method(self, db: &dyn InternDb) -> Option<$ty> {
            match self.loc(db) {
                DefOriginLoc::$variant(id) => Some(id),
                _ => None,
            }
        }
    };
}

impl DefOrigin {
    impl_origin_cast!(as_module, Module, ModuleId);

    impl_origin_cast!(as_config, Config, InFile<ConfigDeclId>);

    impl_origin_cast!(as_library, Library, InFile<LibraryDeclId>);

    impl_origin_cast!(as_udp, Udp, InFile<UdpDeclId>);

    impl_origin_cast!(as_block, Block, BlockId);

    impl_origin_cast!(as_generate_block, GenerateBlock, GenerateBlockId);

    impl_origin_cast!(as_subroutine, Subroutine, SubroutineScope);

    impl_origin_cast!(as_subroutine_port, SubroutinePort, InSubroutine<SubroutinePortId>);

    impl_origin_cast!(as_non_ansi_port, NonAnsiPort, InModule<NonAnsiPortId>);

    impl_origin_cast!(as_decl, Decl, InContainer<DeclId>);

    impl_origin_cast!(as_typedef, Typedef, InContainer<TypedefId>);

    impl_origin_cast!(as_instance, Instance, InModule<InstanceId>);

    impl_origin_cast!(as_modport, Modport, InModule<ModportId>);

    impl_origin_cast!(as_clocking_block, ClockingBlock, InModule<ClockingBlockId>);

    impl_origin_cast!(as_clocking_signal, ClockingSignal, InScope<ClockingSignalId>);

    impl_origin_cast!(as_checker, Checker, InFileOrModule<CheckerId>);

    impl_origin_cast!(as_checker_port, CheckerPort, InScope<CheckerPortId>);

    impl_origin_cast!(as_covergroup, Covergroup, InFileOrModule<CovergroupId>);

    impl_origin_cast!(as_stmt, Stmt, InContainer<StmtId>);

    pub fn new(db: &dyn InternDb, loc: impl Into<DefOriginLoc>) -> Self {
        db.intern_def_origin(loc.into())
    }

    pub fn loc(self, db: &dyn InternDb) -> DefOriginLoc {
        db.lookup_intern_def_origin(self)
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

    pub fn map<U: Eq>(self, map: impl FnMut(T) -> U) -> Resolution<U> {
        Resolution::from_candidates(self.into_candidates().into_iter().map(map))
    }
}

impl<T: Copy> Resolution<T> {
    pub fn unique(&self) -> Option<T> {
        match self {
            Resolution::Unique(value) => Some(*value),
            Resolution::Ambiguous(_) | Resolution::Unresolved => None,
        }
    }

    /// Resolves children without allowing child existence to disambiguate an
    /// ambiguous parent.
    pub fn and_then<U: Eq>(&self, mut resolve: impl FnMut(T) -> Resolution<U>) -> Resolution<U> {
        let children = Resolution::from_candidates(
            self.iter().copied().flat_map(|candidate| resolve(candidate).into_candidates()),
        );
        match (self, children) {
            (Resolution::Ambiguous(_), Resolution::Unique(_)) => Resolution::Unresolved,
            (_, children) => children,
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
            .filter_map(|def_id| def_id.primary_origin(db).as_module(db))
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
            .filter_map(|def_id| def_id.primary_origin(db).as_module(db))
            .collect::<SmallVec<[_; 2]>>();
        Resolution::from_candidates(entries)
    }

    pub fn module_names<'a>(&'a self, db: &'a dyn HirDb) -> impl Iterator<Item = &'a Ident> + 'a {
        self.types.iter().filter_map(move |(ident, defs)| {
            defs.iter()
                .any(|def_id| {
                    def_id.kind(db).is_instantiable_def()
                        && def_id.primary_origin(db).as_module(db).is_some()
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

#[cfg(test)]
mod tests {
    use super::Resolution;

    #[test]
    fn resolution_map_deduplicates_candidates() {
        let resolution = Resolution::from_candidates([1, 2]).map(|_| 0);
        assert_eq!(resolution, Resolution::Unique(0));
    }
}
