use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use utils::impl_from;

use crate::{
    Ident,
    checker::{CheckerId, CheckerPortId},
    container::{
        InContainer, InFile, InFileOrModule, InModule, InScope, InSubroutine, ScopeId,
        SubroutineScope,
    },
    covergroup::{CovergroupId, CoverpointId, CrossId},
    db::HirDefDb,
    def_id::DefId,
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
    owner::OwnerId,
    stmt::StmtId,
    subroutine::SubroutinePortId,
    typedef::TypedefId,
};

/// A concrete source representation of a semantic definition.
///
/// Origins are interned in the Salsa database rather than in a process-global
/// pool. Their identity is therefore scoped to the database that owns the
/// corresponding HIR.
#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
pub struct DefOrigin {
    pub loc: DefOriginLoc,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum DefOriginLoc {
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Block(OwnerId),
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
    Block(OwnerId),
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
        pub fn $method(&self, db: &dyn HirDefDb) -> Option<$ty> {
            match self.loc(db) {
                DefOriginLoc::$variant(id) => Some(id.clone()),
                _ => None,
            }
        }
    };
}

/// Generates `DefOriginLoc::trivial_kind`: the `DefKind` of every variant
/// whose kind does not depend on the database. Variant names mirror
/// `DefKind` names one-to-one; `Module` and `Decl` are excluded because their
/// kind is derived from the lowered data.
macro_rules! trivial_kind {
    ($($variant:ident),* $(,)?) => {
        pub fn trivial_kind(&self) -> DefKind {
            match self {
                $(DefOriginLoc::$variant(_) => DefKind::$variant,)*
                DefOriginLoc::Module(_) | DefOriginLoc::Decl(_) => {
                    unreachable!("kind requires the database")
                }
            }
        }
    };
}

impl DefOriginLoc {
    trivial_kind! {
        Config, Library, Udp, Block, GenerateBlock, Subroutine, SubroutinePort, NonAnsiPort,
        Typedef, Instance, Modport, ClockingBlock, ClockingSignal, Checker, CheckerPort,
        Covergroup, Coverpoint, Cross, Stmt,
    }

    /// The scope that owns this origin. Does not need the database: every
    /// variant carries its container directly.
    pub fn container_id(&self, db: &dyn HirDefDb) -> ScopeId {
        match self {
            DefOriginLoc::Module(InFile { file_id, .. }) => (*file_id).into(),
            DefOriginLoc::Config(InFile { file_id, .. }) => (*file_id).into(),
            DefOriginLoc::Library(InFile { file_id, .. }) => (*file_id).into(),
            DefOriginLoc::Udp(InFile { file_id, .. }) => (*file_id).into(),
            DefOriginLoc::Block(owner) => {
                owner.parent(db).map(ScopeId::Owner).unwrap_or(ScopeId::Owner(*owner))
            }
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                generate_block_id.loc().cont_id.clone().into()
            }
            DefOriginLoc::Subroutine(subroutine_id) => subroutine_id.cont_id.clone().into(),
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ScopeId::Subroutine(subroutine.clone())
            }
            DefOriginLoc::NonAnsiPort(InModule { module_id, .. }) => (*module_id).into(),
            DefOriginLoc::Decl(InContainer { cont_id, .. }) => cont_id.clone().into(),
            DefOriginLoc::Typedef(InContainer { cont_id, .. }) => cont_id.clone().into(),
            DefOriginLoc::Instance(InModule { module_id, .. }) => (*module_id).into(),
            DefOriginLoc::Modport(InModule { module_id, .. }) => (*module_id).into(),
            DefOriginLoc::ClockingBlock(InModule { module_id, .. }) => (*module_id).into(),
            DefOriginLoc::ClockingSignal(InScope { scope_id, .. }) => scope_id.clone(),
            DefOriginLoc::Checker(InFileOrModule { cont_id, .. }) => cont_id.clone().into(),
            DefOriginLoc::CheckerPort(InScope { scope_id, .. }) => scope_id.clone(),
            DefOriginLoc::Covergroup(InFileOrModule { cont_id, .. }) => cont_id.clone().into(),
            DefOriginLoc::Coverpoint(InScope { scope_id, .. }) => scope_id.clone(),
            DefOriginLoc::Cross(InScope { scope_id, .. }) => scope_id.clone(),
            DefOriginLoc::Stmt(InContainer { cont_id, .. }) => cont_id.clone().into(),
        }
    }
}

impl DefOrigin {
    impl_origin_cast!(as_module, Module, ModuleId);

    impl_origin_cast!(as_config, Config, InFile<ConfigDeclId>);

    impl_origin_cast!(as_library, Library, InFile<LibraryDeclId>);

    impl_origin_cast!(as_udp, Udp, InFile<UdpDeclId>);

    impl_origin_cast!(as_block, Block, OwnerId);

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
}

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
            DefKind::Module | DefKind::Package | DefKind::Program => SymbolKind::Module,
            DefKind::Interface => SymbolKind::Interface,
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
    ProceduralBlock,
    Covergroup,
    ClockingBlock,
    Checker,
}

/// Names visible in one lexical scope.
///
/// Each namespace stores all candidates for a name. A name becomes
/// `Resolution::Ambiguous` only when it has multiple distinct `DefId`s;
/// multiple origins of one `DefId` are already merged by canonical identity.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameScope {
    types: FxHashMap<Ident, SmallVec<[DefId; 1]>>,
    values: FxHashMap<Ident, SmallVec<[DefId; 1]>>,
    assertions: FxHashMap<Ident, SmallVec<[DefId; 1]>>,
    imports: SmallVec<[Import; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Import {
    pub package: Ident,
    pub name: Option<Ident>,
}

/// Namespace selected by a lookup.
///
/// `Listing` is the explicit editor-facing union of type and value names.
/// Assertion declarations remain in their own bucket and are not returned by
/// `Listing` until assertion lookup is part of the public resolver contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameContext {
    Type,
    Value,
    Listing,
}

/// A lookup result that preserves the difference between no match, one
/// logical definition, and several competing definitions.
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

impl<T: Clone> Resolution<T> {
    pub fn unique(&self) -> Option<T> {
        match self {
            Resolution::Unique(value) => Some(value.clone()),
            Resolution::Ambiguous(_) | Resolution::Unresolved => None,
        }
    }

    /// Resolves children without allowing child existence to disambiguate an
    /// ambiguous parent.
    pub fn and_then<U: Eq>(&self, mut resolve: impl FnMut(T) -> Resolution<U>) -> Resolution<U> {
        let children = Resolution::from_candidates(
            self.iter().cloned().flat_map(|candidate| resolve(candidate).into_candidates()),
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
    pub fn imports(&self) -> &[Import] {
        &self.imports
    }

    pub fn has_imports(&self) -> bool {
        !self.imports.is_empty()
    }

    pub(crate) fn insert_import(&mut self, import: Import) {
        self.imports.push(import);
    }

    pub(crate) fn extend_definitions_from(&mut self, other: &NameScope) {
        for (ident, defs) in &other.types {
            for def_id in defs {
                self.insert_type(ident, *def_id);
            }
        }
        for (ident, defs) in &other.values {
            for def_id in defs {
                self.insert_value(ident, *def_id);
            }
        }
        for (ident, defs) in &other.assertions {
            for def_id in defs {
                self.insert_assertion(ident, *def_id);
            }
        }
    }

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
            defs.extend(type_defs.iter().copied());
        }
        if let Some(value_defs) = self.values.get(ident) {
            defs.extend(value_defs.iter().copied());
        }
        defs
    }

    pub fn iter_listing(&self) -> impl Iterator<Item = (&Ident, SmallVec<[DefId; 1]>)> + '_ {
        self.types
            .iter()
            .map(|(ident, type_defs)| {
                let mut defs = type_defs.iter().copied().collect::<SmallVec<[DefId; 1]>>();
                if let Some(value_defs) = self.values.get(ident) {
                    defs.extend(value_defs.iter().copied());
                }
                (ident, defs)
            })
            .chain(
                self.values
                    .iter()
                    .filter(|(ident, _)| !self.types.contains_key(*ident))
                    .map(|(ident, defs)| (ident, defs.iter().copied().collect())),
            )
    }

    pub fn module_ids(
        &self,
        db: &dyn HirDefDb,
        ident: &Ident,
    ) -> Resolution<crate::module::ModuleId> {
        let entries = self
            .types
            .get(ident)
            .into_iter()
            .flat_map(|defs| defs.iter())
            .filter(|def_id| def_id.kind(db).is_instantiable_def())
            .filter_map(|def_id| def_id.primary_origin(db).as_module(db));
        Resolution::from_candidates(entries)
    }

    pub fn package_ids(
        &self,
        db: &dyn HirDefDb,
        ident: &Ident,
    ) -> Resolution<crate::module::PackageId> {
        let entries = self
            .types
            .get(ident)
            .into_iter()
            .flat_map(|defs| defs.iter())
            .filter(|def_id| def_id.kind(db) == DefKind::Package)
            .filter_map(|def_id| def_id.primary_origin(db).as_module(db));
        Resolution::from_candidates(entries)
    }

    pub fn module_names<'a>(
        &'a self,
        db: &'a dyn HirDefDb,
    ) -> impl Iterator<Item = &'a Ident> + 'a {
        self.types.iter().filter_map(move |(ident, defs)| {
            defs.iter()
                .any(|def_id| {
                    def_id.kind(db).is_instantiable_def()
                        && def_id.primary_origin(db).as_module(db).is_some()
                })
                .then_some(ident)
        })
    }

    pub fn typedef_names<'a>(
        &'a self,
        db: &'a dyn HirDefDb,
    ) -> impl Iterator<Item = &'a Ident> + 'a {
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

    #[test]
    fn resolution_does_not_let_one_ambiguous_parent_become_unique() {
        let parent = Resolution::from_candidates([1, 2]);
        let child = parent.and_then(|candidate| {
            if candidate == 1 { Resolution::Unique("child") } else { Resolution::Unresolved }
        });

        assert_eq!(child, Resolution::Unresolved);
    }
}
