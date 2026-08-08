use base_db::salsa;
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use salsa::plumbing::AsId;
use smallvec::SmallVec;
use smol_str::SmolStr;
use triomphe::Arc;
use utils::{get::GetRef, text_edit::TextRange};

use crate::{
    ast_id_map::SourceAstId,
    checker::{CheckerDef, CheckerPort, CheckerPortId},
    container::{InFile, OwnerRef},
    covergroup::{CoverpointDef, CoverpointId, CrossDef, CrossId},
    db::HirDefDb,
    declaration::Declaration,
    expr::declarator::DeclaratorParent,
    module::ModuleKind,
    owner::{OwnerId, OwnerKind},
    subroutine::SubroutinePortId,
    symbol::{DefKind, DefOrigin, DefOriginLoc},
};

fn clocking_signal_of(
    db: &dyn HirDefDb,
    signal: OwnerRef<crate::module::clocking::ClockingSignalId>,
) -> Option<(OwnerRef<crate::module::clocking::ClockingSignal>, HirFileId)> {
    let clocking = signal.cont_id.as_clocking_block(db)?;
    let body = db.body(signal.cont_id);
    let block = GetRef::get(body.as_ref(), clocking.value);
    let value = block.signals.get(signal.value.0 as usize)?.clone();
    Some((OwnerRef::new(signal.cont_id, value), signal.cont_id.file(db)))
}

fn checker_of(
    db: &dyn HirDefDb,
    checker: OwnerRef<crate::checker::CheckerId>,
) -> Option<(CheckerDef, HirFileId)> {
    let file_id = checker.cont_id.file(db);
    Some((GetRef::get(db.body(checker.cont_id).as_ref(), checker.value).clone(), file_id))
}

fn checker_port_of(
    db: &dyn HirDefDb,
    port: OwnerRef<CheckerPortId>,
) -> Option<(CheckerPort, HirFileId)> {
    let checker = port.cont_id.as_checker(db)?;
    let (checker, file_id) = checker_of(db, checker)?;
    let port = checker.ports.get(port.value.0 as usize)?.clone();
    Some((port, file_id))
}

fn coverpoint_of(
    db: &dyn HirDefDb,
    coverpoint: OwnerRef<CoverpointId>,
) -> Option<(CoverpointDef, HirFileId)> {
    Some((
        GetRef::get(db.body(coverpoint.cont_id).as_ref(), coverpoint.value).clone(),
        coverpoint.cont_id.file(db),
    ))
}

fn cross_of(db: &dyn HirDefDb, cross: OwnerRef<CrossId>) -> Option<(CrossDef, HirFileId)> {
    Some((
        GetRef::get(db.body(cross.cont_id).as_ref(), cross.value).clone(),
        cross.cont_id.file(db),
    ))
}

impl DefOriginLoc {
    /// Canonical owner for this source definition.
    pub fn owner(self, db: &dyn HirDefDb) -> OwnerId {
        definition_owner(db, &self)
    }

    pub fn kind(self, db: &dyn HirDefDb) -> DefKind {
        match self {
            DefOriginLoc::Module(owner) => {
                match owner.module_kind(db).expect("module owner kind") {
                    ModuleKind::Module => DefKind::Module,
                    ModuleKind::Interface => DefKind::Interface,
                    ModuleKind::Program => DefKind::Program,
                    ModuleKind::Package => DefKind::Package,
                }
            }
            DefOriginLoc::Decl(OwnerRef { value, cont_id }) => {
                let container = cont_id.data(db);
                let decl = container.declarator(value);
                match decl.parent {
                    DeclaratorParent::PortDeclId(_) => DefKind::Port,
                    DeclaratorParent::StmtId(_) => DefKind::Variable,
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        match container.declaration(declaration_id) {
                            Declaration::DataDecl(_) => DefKind::Variable,
                            Declaration::NetDecl(_) => DefKind::Net,
                            Declaration::ParamDecl(_) => DefKind::Param,
                            Declaration::GenvarDecl(_) => DefKind::Genvar,
                            Declaration::SpecparamDecl(_) => DefKind::Specparam,
                        }
                    }
                }
            }
            _ => self.trivial_kind(),
        }
    }

    pub fn name(self, db: &dyn HirDefDb) -> Option<SmolStr> {
        match self {
            DefOriginLoc::Module(owner) => db.body(owner).name.clone(),
            DefOriginLoc::Config(InFile { value, file_id }) => GetRef::get(
                db.body(db.owner_table(file_id).file_owner().expect("file owner")).as_ref(),
                value,
            )
            .name
            .clone(),
            DefOriginLoc::Library(InFile { value, file_id }) => GetRef::get(
                db.body(db.owner_table(file_id).file_owner().expect("file owner")).as_ref(),
                value,
            )
            .name
            .clone(),
            DefOriginLoc::Udp(InFile { value, file_id }) => GetRef::get(
                db.body(db.owner_table(file_id).file_owner().expect("file owner")).as_ref(),
                value,
            )
            .name
            .clone(),
            DefOriginLoc::Block(owner) => owner.name(db),
            DefOriginLoc::GenerateBlock(owner) => db.body(owner).name.clone(),
            DefOriginLoc::Subroutine(owner) => db.subroutine(owner).name.clone(),
            DefOriginLoc::SubroutinePort(OwnerRef { cont_id: subroutine, value }) => {
                db.subroutine(subroutine).ports.get(value.0 as usize)?.name.clone()
            }
            DefOriginLoc::NonAnsiPort(port) => {
                GetRef::get(port.cont_id.data(db).as_ref(), port.value).label.clone()
            }
            DefOriginLoc::Decl(OwnerRef { value, cont_id }) => {
                cont_id.data(db).declarator(value).name.clone()
            }
            DefOriginLoc::Typedef(OwnerRef { value, cont_id }) => {
                cont_id.data(db).typedef(value).name.clone()
            }
            DefOriginLoc::Instance(OwnerRef { value, cont_id }) => {
                GetRef::get(db.body(cont_id).as_ref(), value).name.clone()
            }
            DefOriginLoc::Modport(OwnerRef { value, cont_id }) => {
                GetRef::get(db.body(cont_id).as_ref(), value).name.clone()
            }
            DefOriginLoc::ClockingBlock(OwnerRef { value, cont_id }) => {
                GetRef::get(db.body(cont_id).as_ref(), value).name.clone()
            }
            DefOriginLoc::ClockingSignal(signal) => {
                clocking_signal_of(db, signal).map(|(signal, _)| signal.value.name)
            }
            DefOriginLoc::Checker(OwnerRef { value, cont_id }) => {
                GetRef::get(db.body(cont_id).as_ref(), value).name.clone()
            }
            DefOriginLoc::Covergroup(OwnerRef { value, cont_id }) => {
                GetRef::get(db.body(cont_id).as_ref(), value).name.clone()
            }
            DefOriginLoc::CheckerPort(port) => checker_port_of(db, port).map(|(port, _)| port.name),
            DefOriginLoc::Coverpoint(coverpoint) => {
                coverpoint_of(db, coverpoint).and_then(|(coverpoint, _)| coverpoint.name)
            }
            DefOriginLoc::Cross(cross) => cross_of(db, cross).and_then(|(cross, _)| cross.name),
            DefOriginLoc::Stmt(OwnerRef { value, cont_id }) => {
                cont_id.data(db).stmt(value).label.clone()
            }
        }
    }

    pub(crate) fn source_ast(self, db: &dyn HirDefDb) -> Option<InFile<SourceAstId>> {
        fn owner_source<Id>(
            db: &dyn HirDefDb,
            owner: OwnerId,
            id: Id,
        ) -> Option<InFile<SourceAstId>>
        where
            crate::body::BodySourceMap: utils::get::Get<Id, Output = Option<SourceAstId>>,
        {
            let source = db.body_with_source_map(owner).source(id)?;
            Some(InFile::new(owner.file(db), source))
        }

        match self {
            DefOriginLoc::Module(owner) => Some(InFile::new(owner.file(db), owner.ast_id(db))),
            DefOriginLoc::Config(InFile { value, file_id }) => {
                owner_source(db, db.owner_table(file_id).file_owner()?, value)
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                owner_source(db, db.owner_table(file_id).file_owner()?, value)
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                owner_source(db, db.owner_table(file_id).file_owner()?, value)
            }
            DefOriginLoc::Block(owner) => Some(InFile::new(owner.file(db), owner.ast_id(db))),
            DefOriginLoc::GenerateBlock(owner) => {
                Some(InFile::new(owner.file(db), owner.ast_id(db)))
            }
            DefOriginLoc::Subroutine(owner) => Some(InFile::new(owner.file(db), owner.ast_id(db))),
            DefOriginLoc::SubroutinePort(OwnerRef { cont_id: subroutine, value }) => {
                let source = db.subroutine(subroutine).ports.get(value.0 as usize)?.source;
                Some(InFile::new(subroutine.file(db), source))
            }
            DefOriginLoc::NonAnsiPort(OwnerRef { value, cont_id }) => {
                owner_source(db, cont_id, value)
            }
            DefOriginLoc::Instance(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::Modport(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::ClockingBlock(OwnerRef { value, cont_id }) => {
                owner_source(db, cont_id, value)
            }
            DefOriginLoc::Decl(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::Typedef(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::Stmt(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::ClockingSignal(signal) => {
                let (signal, file_id) = clocking_signal_of(db, signal)?;
                Some(InFile::new(file_id, signal.value.source))
            }
            DefOriginLoc::Checker(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::Covergroup(OwnerRef { value, cont_id }) => {
                owner_source(db, cont_id, value)
            }
            DefOriginLoc::CheckerPort(port) => {
                let (port, file_id) = checker_port_of(db, port)?;
                Some(InFile::new(file_id, port.source))
            }
            DefOriginLoc::Coverpoint(point) => owner_source(db, point.cont_id, point.value),
            DefOriginLoc::Cross(cross) => owner_source(db, cross.cont_id, cross.value),
        }
    }

    pub fn name_range(self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        let source = self.source_ast(db)?;
        let range = db.source_projection(source.file_id).origin(source.value)?.focus_range()?;
        Some(InFile::new(source.file_id, range))
    }

    pub fn range(self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        let source = self.source_ast(db)?;
        let range = db.source_projection(source.file_id).origin(source.value)?.full_range()?;
        Some(InFile::new(source.file_id, range))
    }
}

impl DefOrigin {
    #[inline]
    pub fn container_id(&self, db: &dyn HirDefDb) -> OwnerId {
        self.loc(db).clone().container_id(db)
    }

    pub fn kind(&self, db: &dyn HirDefDb) -> DefKind {
        self.loc(db).clone().kind(db)
    }

    pub fn name(&self, db: &dyn HirDefDb) -> Option<SmolStr> {
        self.loc(db).clone().name(db)
    }

    pub fn name_range(&self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        self.loc(db).clone().name_range(db)
    }

    pub fn range(&self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        self.loc(db).clone().range(db)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum DefinitionRole {
    Module,
    Config,
    Library,
    Udp,
    Block,
    GenerateBlock,
    Subroutine,
    SubroutinePort,
    NonAnsiPort,
    Decl,
    Typedef,
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

impl DefinitionRole {
    fn of(loc: &DefOriginLoc) -> Self {
        match loc {
            DefOriginLoc::Module(_) => Self::Module,
            DefOriginLoc::Config(_) => Self::Config,
            DefOriginLoc::Library(_) => Self::Library,
            DefOriginLoc::Udp(_) => Self::Udp,
            DefOriginLoc::Block(_) => Self::Block,
            DefOriginLoc::GenerateBlock(_) => Self::GenerateBlock,
            DefOriginLoc::Subroutine(_) => Self::Subroutine,
            DefOriginLoc::SubroutinePort(_) => Self::SubroutinePort,
            DefOriginLoc::NonAnsiPort(_) => Self::NonAnsiPort,
            DefOriginLoc::Decl(_) => Self::Decl,
            DefOriginLoc::Typedef(_) => Self::Typedef,
            DefOriginLoc::Instance(_) => Self::Instance,
            DefOriginLoc::Modport(_) => Self::Modport,
            DefOriginLoc::ClockingBlock(_) => Self::ClockingBlock,
            DefOriginLoc::ClockingSignal(_) => Self::ClockingSignal,
            DefOriginLoc::Checker(_) => Self::Checker,
            DefOriginLoc::CheckerPort(_) => Self::CheckerPort,
            DefOriginLoc::Covergroup(_) => Self::Covergroup,
            DefOriginLoc::Coverpoint(_) => Self::Coverpoint,
            DefOriginLoc::Cross(_) => Self::Cross,
            DefOriginLoc::Stmt(_) => Self::Stmt,
        }
    }
}

/// Stable ordinal allocated once while collecting definitions for one owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalDefId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DefinitionKey {
    source: SourceAstId,
    role: DefinitionRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DefinitionData {
    primary: DefOriginLoc,
    additional: SmallVec<[DefOriginLoc; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DefinitionTable {
    definitions: Vec<DefinitionData>,
    by_key: FxHashMap<DefinitionKey, LocalDefId>,
}

impl DefinitionTable {
    fn insert(&mut self, source: SourceAstId, primary: DefOriginLoc) -> LocalDefId {
        let key = DefinitionKey { source, role: DefinitionRole::of(&primary) };
        assert!(
            !self.by_key.contains_key(&key),
            "definition key inserted twice: {source:?} {:?}",
            key.role
        );
        let local = LocalDefId(self.definitions.len() as u32);
        self.definitions.push(DefinitionData { primary, additional: SmallVec::new() });
        self.by_key.insert(key, local);
        local
    }

    fn alias(&mut self, source: SourceAstId, origin: DefOriginLoc, local: LocalDefId) {
        let key = DefinitionKey { source, role: DefinitionRole::of(&origin) };
        let previous = self.by_key.insert(key, local);
        assert!(previous.is_none(), "definition alias inserted twice");
        self.definitions[local.0 as usize].additional.push(origin);
    }

    fn local_for(&self, source: SourceAstId, role: DefinitionRole) -> Option<LocalDefId> {
        self.by_key.get(&DefinitionKey { source, role }).copied()
    }

    fn get(&self, local: LocalDefId) -> Option<&DefinitionData> {
        self.definitions.get(local.0 as usize)
    }
}

#[salsa::tracked(lru = 512, returns(clone))]
pub(crate) fn definition_table(db: &dyn HirDefDb, owner: OwnerId) -> Arc<DefinitionTable> {
    let lowered = db.body_with_source_map(owner);
    let body = lowered.data_ref();
    let sources = lowered.source_map();
    let mut table = DefinitionTable::default();

    match owner.kind(db) {
        OwnerKind::Module => {
            table.insert(owner.ast_id(db), DefOriginLoc::Module(owner));
            if let crate::module::port::PortSrcs::NonAnsi { ports, .. } = &sources.port_srcs {
                for (port, source) in ports.iter() {
                    table.insert(source, DefOriginLoc::NonAnsiPort(OwnerRef::new(owner, port)));
                }
            }
            for (id, source) in sources.instance_srcs.iter() {
                table.insert(source, DefOriginLoc::Instance(OwnerRef::new(owner, id)));
            }
            for (id, source) in sources.modport_srcs.iter() {
                table.insert(source, DefOriginLoc::Modport(OwnerRef::new(owner, id)));
            }
            for (id, source) in sources.clocking_block_srcs.iter() {
                table.insert(source, DefOriginLoc::ClockingBlock(OwnerRef::new(owner, id)));
            }
        }
        OwnerKind::GenerateBlock => {
            table.insert(owner.ast_id(db), DefOriginLoc::GenerateBlock(owner));
        }
        OwnerKind::Block => {
            table.insert(owner.ast_id(db), DefOriginLoc::Block(owner));
        }
        OwnerKind::Subroutine => {
            table.insert(owner.ast_id(db), DefOriginLoc::Subroutine(owner));
            let subroutine = db.subroutine(owner);
            for (index, port) in subroutine.ports.iter().enumerate() {
                let port_id = SubroutinePortId(index as u32);
                table.insert(
                    port.source,
                    DefOriginLoc::SubroutinePort(OwnerRef::new(owner, port_id)),
                );
            }
        }
        OwnerKind::Checker => {
            let checker = owner.as_checker(db).expect("checker owner must contain a definition");
            let source = sources.checker_srcs.hir_to_src(checker.value).expect("checker source");
            table.insert(source, DefOriginLoc::Checker(checker));
            let checker_data = GetRef::get(body, checker.value);
            for (index, port) in checker_data.ports.iter().enumerate() {
                let port_id = CheckerPortId(index as u32);
                table.insert(port.source, DefOriginLoc::CheckerPort(OwnerRef::new(owner, port_id)));
            }
        }
        OwnerKind::Covergroup => {
            let covergroup =
                owner.as_covergroup(db).expect("covergroup owner must contain a definition");
            let source =
                sources.covergroup_srcs.hir_to_src(covergroup.value).expect("covergroup source");
            table.insert(source, DefOriginLoc::Covergroup(covergroup));
            for (id, source) in sources.coverpoint_srcs.iter() {
                table.insert(source, DefOriginLoc::Coverpoint(OwnerRef::new(owner, id)));
            }
            for (id, source) in sources.cross_srcs.iter() {
                table.insert(source, DefOriginLoc::Cross(OwnerRef::new(owner, id)));
            }
        }
        OwnerKind::ClockingBlock => {
            let clocking =
                owner.as_clocking_block(db).expect("clocking owner must contain a definition");
            let source =
                sources.clocking_block_srcs.hir_to_src(clocking.value).expect("clocking source");
            table.insert(source, DefOriginLoc::ClockingBlock(clocking));
            let block = GetRef::get(body, clocking.value);
            for (index, signal) in block.signals.iter().enumerate() {
                let signal_id = crate::module::clocking::ClockingSignalId(index as u32);
                table.insert(
                    signal.source,
                    DefOriginLoc::ClockingSignal(OwnerRef::new(owner, signal_id)),
                );
            }
        }
        OwnerKind::File | OwnerKind::ProceduralBlock => {}
    }

    for (id, source) in sources.config_decl_srcs.iter() {
        table.insert(source, DefOriginLoc::Config(InFile::new(owner.file(db), id)));
    }
    for (id, source) in sources.library_decl_srcs.iter() {
        table.insert(source, DefOriginLoc::Library(InFile::new(owner.file(db), id)));
    }
    for (id, source) in sources.udp_decl_srcs.iter() {
        table.insert(source, DefOriginLoc::Udp(InFile::new(owner.file(db), id)));
    }

    if let Some(scope) = body.scope(owner) {
        for id in scope.declarators() {
            let source = sources.decl_srcs.hir_to_src(*id).expect("declarator source");
            let origin = DefOriginLoc::Decl(OwnerRef::new(owner, *id));
            if let crate::module::port::Ports::NonAnsi { bindings, .. } = &body.ports {
                if let Some(port) = bindings.decl_to_port.get(id).copied() {
                    let port_source = match &sources.port_srcs {
                        crate::module::port::PortSrcs::NonAnsi { ports, .. } => {
                            ports.hir_to_src(port).expect("non-ANSI port source")
                        }
                        crate::module::port::PortSrcs::Ansi { .. } => {
                            unreachable!("non-ANSI bindings require non-ANSI sources")
                        }
                    };
                    let local = table
                        .local_for(port_source, DefinitionRole::NonAnsiPort)
                        .expect("non-ANSI port must be collected before declarations");
                    table.alias(source, origin, local);
                    continue;
                }
            }
            table.insert(source, origin);
        }
        for id in scope.typedefs() {
            let source = sources.typedef_srcs.hir_to_src(*id).expect("typedef source");
            table.insert(source, DefOriginLoc::Typedef(OwnerRef::new(owner, *id)));
        }
        for id in scope.statements() {
            let source = sources.stmt_srcs.hir_to_src(*id).expect("statement source");
            table.insert(source, DefOriginLoc::Stmt(OwnerRef::new(owner, *id)));
        }
    }

    Arc::new(table)
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
struct InternedDefId {
    #[returns(copy)]
    owner: OwnerId,
    #[returns(copy)]
    local: LocalDefId,
}

/// Canonical definition identity: an owner plus a collected owner-local id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefId(InternedDefId);

impl PartialOrd for DefId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DefId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_id().cmp(&other.0.as_id())
    }
}

impl DefId {
    pub fn new(db: &dyn HirDefDb, loc: impl Into<DefOriginLoc>) -> Self {
        let loc = loc.into();
        let owner = loc.clone().owner(db);
        let source = loc
            .clone()
            .source_ast(db)
            .expect("every semantic definition must have a source AST identity")
            .value;
        let role = DefinitionRole::of(&loc);
        let local = db
            .definition_table(owner)
            .local_for(source, role)
            .expect("definition origin must be collected by its owner");
        Self(InternedDefId::new(db, owner, local))
    }

    pub fn origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 3]> {
        let table = db.definition_table(self.0.owner(db));
        let data = table
            .get(self.0.local(db))
            .expect("definition local id must project into its owner table");
        let mut origins = SmallVec::new();
        origins.push(DefOrigin::new(db, data.primary.clone()));
        origins.extend(data.additional.iter().cloned().map(|loc| DefOrigin::new(db, loc)));
        origins
    }

    pub fn primary_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        let table = db.definition_table(self.0.owner(db));
        let data = table
            .get(self.0.local(db))
            .expect("definition local id must project into its owner table");
        DefOrigin::new(db, data.primary.clone())
    }

    pub fn declaration_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port(db).is_some() {
            return self
                .origins(db)
                .into_iter()
                .find(|origin| is_port_decl_origin(db, *origin))
                .unwrap_or(primary_origin);
        }
        primary_origin
    }

    pub fn declaration_origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 2]> {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port(db).is_some() {
            return self
                .origins(db)
                .into_iter()
                .filter(|origin| matches!(origin.loc(db), DefOriginLoc::Decl(_)))
                .collect();
        }
        let mut origins = SmallVec::new();
        origins.push(primary_origin);
        origins
    }

    pub fn is_non_ansi_port(&self, db: &dyn HirDefDb) -> bool {
        matches!(self.primary_origin(db).loc(db), DefOriginLoc::NonAnsiPort(_))
    }

    pub fn is_port(&self, db: &dyn HirDefDb) -> bool {
        self.is_non_ansi_port(db)
            || self.origins(db).iter().any(|origin| is_port_decl_origin(db, *origin))
    }

    pub fn container_id(&self, db: &dyn HirDefDb) -> OwnerId {
        self.primary_origin(db).container_id(db)
    }

    pub fn kind(&self, db: &dyn HirDefDb) -> DefKind {
        if self.is_non_ansi_port(db) {
            self.declaration_origin(db).kind(db)
        } else {
            self.primary_origin(db).kind(db)
        }
    }

    pub fn name(&self, db: &dyn HirDefDb) -> Option<SmolStr> {
        self.primary_origin(db).name(db)
    }
}
fn definition_owner(db: &dyn HirDefDb, loc: &DefOriginLoc) -> OwnerId {
    match loc {
        DefOriginLoc::Module(owner)
        | DefOriginLoc::Block(owner)
        | DefOriginLoc::GenerateBlock(owner)
        | DefOriginLoc::Subroutine(owner) => *owner,
        DefOriginLoc::Config(InFile { file_id, .. })
        | DefOriginLoc::Library(InFile { file_id, .. })
        | DefOriginLoc::Udp(InFile { file_id, .. }) => file_owner(db, *file_id),
        DefOriginLoc::SubroutinePort(port) => port.cont_id,
        DefOriginLoc::NonAnsiPort(port) => port.cont_id,
        DefOriginLoc::Decl(decl) => decl.cont_id,
        DefOriginLoc::Typedef(typedef) => typedef.cont_id,
        DefOriginLoc::Instance(instance) => instance.cont_id,
        DefOriginLoc::Modport(modport) => modport.cont_id,
        DefOriginLoc::ClockingBlock(block) => block.cont_id,
        DefOriginLoc::ClockingSignal(signal) => signal.cont_id,
        DefOriginLoc::Checker(checker) => checker.cont_id,
        DefOriginLoc::CheckerPort(port) => port.cont_id,
        DefOriginLoc::Covergroup(covergroup) => covergroup.cont_id,
        DefOriginLoc::Coverpoint(coverpoint) => coverpoint.cont_id,
        DefOriginLoc::Cross(cross) => cross.cont_id,
        DefOriginLoc::Stmt(stmt) => stmt.cont_id,
    }
}

fn file_owner(db: &dyn HirDefDb, file_id: HirFileId) -> OwnerId {
    db.owner_table(file_id).file_owner().expect("file must have a canonical owner")
}

fn is_port_decl_origin(db: &dyn HirDefDb, origin: DefOrigin) -> bool {
    let DefOriginLoc::Decl(decl_id) = origin.loc(db) else {
        return false;
    };
    matches!(
        decl_id.cont_id.data(db).declarator(decl_id.value).parent,
        DeclaratorParent::PortDeclId(_)
    )
}
pub(crate) fn set_definition_table_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    definition_table::set_lru_capacity(db, capacity);
}
