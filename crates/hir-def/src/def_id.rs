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
    expr::{data_ty::DataTy, declarator::DeclaratorParent},
    module::ModuleKind,
    owner::{OwnerId, OwnerKind},
    stmt::{ForInit, StmtKind},
    subroutine::{SubroutineKind, SubroutinePortId},
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
            // Named structural owners keep their identity on the owner table.
            // Reading the name must not lower a body: `$unit` and header
            // intern project these owners into `DefId`s before any body query.
            DefOriginLoc::Module(owner)
            | DefOriginLoc::Block(owner)
            | DefOriginLoc::GenerateBlock(owner)
            | DefOriginLoc::Subroutine(owner) => owner.name(db),
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
            DefOriginLoc::Property(OwnerRef { value, cont_id }) => {
                GetRef::get(db.body(cont_id).as_ref(), value).name.clone()
            }
            DefOriginLoc::Sequence(OwnerRef { value, cont_id }) => {
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
            DefOriginLoc::Property(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
            DefOriginLoc::Sequence(OwnerRef { value, cont_id }) => owner_source(db, cont_id, value),
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
/// Stable identity derived from semantic source order, never a vector ordinal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalDefId(DefinitionKey);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DefinitionNameKey {
    kind: DefKind,
    name: Option<SmolStr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct DefinitionKey {
    name: DefinitionNameKey,
    ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DefinitionData {
    primary: DefOriginLoc,
    additional: SmallVec<[DefOriginLoc; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct DefinitionTable {
    definitions: FxHashMap<LocalDefId, DefinitionData>,
    by_source: FxHashMap<(SourceAstId, DefKind), LocalDefId>,
    next_ordinals: FxHashMap<DefinitionNameKey, u32>,
}

impl DefinitionTable {
    fn allocate(
        &mut self,
        source: SourceAstId,
        kind: DefKind,
        name: Option<SmolStr>,
    ) -> LocalDefId {
        let source_key = (source, kind);
        assert!(
            !self.by_source.contains_key(&source_key),
            "definition source inserted twice: {source:?} {kind:?}"
        );
        let name_key = DefinitionNameKey { kind, name };
        let ordinal = self.next_ordinals.entry(name_key.clone()).or_default();
        let key = DefinitionKey { name: name_key, ordinal: *ordinal };
        *ordinal += 1;
        let local = LocalDefId(key);
        self.by_source.insert(source_key, local.clone());
        local
    }

    fn insert(
        &mut self,
        db: &dyn HirDefDb,
        source: SourceAstId,
        primary: DefOriginLoc,
    ) -> LocalDefId {
        let local = self.allocate(source, primary.clone().kind(db), primary.clone().name(db));
        self.definitions
            .insert(local.clone(), DefinitionData { primary, additional: SmallVec::new() });
        local
    }

    fn alias(
        &mut self,
        db: &dyn HirDefDb,
        source: SourceAstId,
        origin: DefOriginLoc,
        local: LocalDefId,
    ) {
        let kind = origin.clone().kind(db);
        let source_key = (source, kind);
        assert!(
            !self.by_source.contains_key(&source_key),
            "definition source inserted twice: {source:?} {kind:?}"
        );
        let name_key = DefinitionNameKey { kind, name: origin.clone().name(db) };
        *self.next_ordinals.entry(name_key).or_default() += 1;
        self.by_source.insert(source_key, local.clone());
        self.definitions
            .get_mut(&local)
            .expect("definition alias must target an existing definition")
            .additional
            .push(origin);
    }

    fn local_for(&self, source: SourceAstId, kind: DefKind) -> Option<LocalDefId> {
        self.by_source.get(&(source, kind)).cloned()
    }

    fn get(&self, local: LocalDefId) -> Option<&DefinitionData> {
        self.definitions.get(&local)
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
            table.insert(db, owner.ast_id(db), DefOriginLoc::Module(owner));
            if let crate::module::port::PortSrcs::NonAnsi { ports, .. } = &sources.port_srcs {
                for (port, source) in ports.iter() {
                    table.insert(db, source, DefOriginLoc::NonAnsiPort(OwnerRef::new(owner, port)));
                }
            }
            for (id, source) in sources.instance_srcs.iter() {
                table.insert(db, source, DefOriginLoc::Instance(OwnerRef::new(owner, id)));
            }
            for (id, source) in sources.modport_srcs.iter() {
                table.insert(db, source, DefOriginLoc::Modport(OwnerRef::new(owner, id)));
            }
            for (id, source) in sources.clocking_block_srcs.iter() {
                table.insert(db, source, DefOriginLoc::ClockingBlock(OwnerRef::new(owner, id)));
            }
        }
        OwnerKind::GenerateBlock => {
            table.insert(db, owner.ast_id(db), DefOriginLoc::GenerateBlock(owner));
            for (id, source) in sources.instance_srcs.iter() {
                table.insert(db, source, DefOriginLoc::Instance(OwnerRef::new(owner, id)));
            }
        }
        OwnerKind::Block => {
            table.insert(db, owner.ast_id(db), DefOriginLoc::Block(owner));
        }
        OwnerKind::Subroutine => {
            table.insert(db, owner.ast_id(db), DefOriginLoc::Subroutine(owner));
            let subroutine = db.subroutine(owner);
            for (index, port) in subroutine.ports.iter().enumerate() {
                let port_id = SubroutinePortId(index as u32);
                table.insert(
                    db,
                    port.source,
                    DefOriginLoc::SubroutinePort(OwnerRef::new(owner, port_id)),
                );
            }
        }
        OwnerKind::Checker => {
            let checker = owner.as_checker(db).expect("checker owner must contain a definition");
            let source = sources.checker_srcs.hir_to_src(checker.value).expect("checker source");
            table.insert(db, source, DefOriginLoc::Checker(checker));
            let checker_data = GetRef::get(body, checker.value);
            for (index, port) in checker_data.ports.iter().enumerate() {
                let port_id = CheckerPortId(index as u32);
                table.insert(
                    db,
                    port.source,
                    DefOriginLoc::CheckerPort(OwnerRef::new(owner, port_id)),
                );
            }
        }
        OwnerKind::Covergroup => {
            let covergroup =
                owner.as_covergroup(db).expect("covergroup owner must contain a definition");
            let source =
                sources.covergroup_srcs.hir_to_src(covergroup.value).expect("covergroup source");
            table.insert(db, source, DefOriginLoc::Covergroup(covergroup));
            for (id, source) in sources.coverpoint_srcs.iter() {
                table.insert(db, source, DefOriginLoc::Coverpoint(OwnerRef::new(owner, id)));
            }
            for (id, source) in sources.cross_srcs.iter() {
                table.insert(db, source, DefOriginLoc::Cross(OwnerRef::new(owner, id)));
            }
        }
        OwnerKind::ClockingBlock => {
            let clocking =
                owner.as_clocking_block(db).expect("clocking owner must contain a definition");
            let source =
                sources.clocking_block_srcs.hir_to_src(clocking.value).expect("clocking source");
            table.insert(db, source, DefOriginLoc::ClockingBlock(clocking));
            let block = GetRef::get(body, clocking.value);
            for (index, signal) in block.signals.iter().enumerate() {
                let signal_id = crate::module::clocking::ClockingSignalId(index as u32);
                table.insert(
                    db,
                    signal.source,
                    DefOriginLoc::ClockingSignal(OwnerRef::new(owner, signal_id)),
                );
            }
        }
        OwnerKind::AnonymousProgram => {}
        OwnerKind::File | OwnerKind::ProceduralBlock => {}
    }

    for (id, source) in sources.config_decl_srcs.iter() {
        table.insert(db, source, DefOriginLoc::Config(InFile::new(owner.file(db), id)));
    }
    for (id, source) in sources.library_decl_srcs.iter() {
        table.insert(db, source, DefOriginLoc::Library(InFile::new(owner.file(db), id)));
    }
    for (id, source) in sources.udp_decl_srcs.iter() {
        table.insert(db, source, DefOriginLoc::Udp(InFile::new(owner.file(db), id)));
    }
    for (id, source) in sources.property_srcs.iter() {
        table.insert(db, source, DefOriginLoc::Property(OwnerRef::new(owner, id)));
    }
    for (id, source) in sources.sequence_srcs.iter() {
        table.insert(db, source, DefOriginLoc::Sequence(OwnerRef::new(owner, id)));
    }

    if let Some(scope) = body.scope(owner) {
        for id in scope.declarators() {
            let source = sources.decl_srcs.hir_to_src(*id).expect("declarator source");
            let origin = DefOriginLoc::Decl(OwnerRef::new(owner, *id));
            if let crate::module::port::Ports::NonAnsi { bindings, .. } = &body.ports
                && let Some(port) = bindings.decl_to_port.get(id).copied()
            {
                let port_source = match &sources.port_srcs {
                    crate::module::port::PortSrcs::NonAnsi { ports, .. } => {
                        ports.hir_to_src(port).expect("non-ANSI port source")
                    }
                    crate::module::port::PortSrcs::Ansi { .. } => {
                        unreachable!("non-ANSI bindings require non-ANSI sources")
                    }
                };
                let local = table
                    .local_for(port_source, DefKind::NonAnsiPort)
                    .expect("non-ANSI port must be collected before declarations");
                table.alias(db, source, origin, local);
                continue;
            }
            table.insert(db, source, origin);
        }
        for id in scope.typedefs() {
            let source = sources.typedef_srcs.hir_to_src(*id).expect("typedef source");
            table.insert(db, source, DefOriginLoc::Typedef(OwnerRef::new(owner, *id)));
        }
        for id in scope.statements() {
            let source = sources.stmt_srcs.hir_to_src(*id).expect("statement source");
            table.insert(db, source, DefOriginLoc::Stmt(OwnerRef::new(owner, *id)));
        }
    }

    Arc::new(table)
}

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
struct InternedDefId {
    #[returns(copy)]
    owner: OwnerId,
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
    fn intern(db: &dyn HirDefDb, loc: impl Into<DefOriginLoc>) -> Self {
        let loc = loc.into();
        let owner = loc.clone().owner(db);
        let source = loc
            .clone()
            .source_ast(db)
            .expect("every semantic definition must have a source AST identity")
            .value;
        let kind = loc.clone().kind(db);
        let local = db
            .definition_table(owner)
            .local_for(source, kind)
            .expect("definition origin must be collected by its owner");
        Self(InternedDefId::new(db, owner, local))
    }

    /// Project a named structural owner into its canonical definition.
    ///
    /// The owner seam deliberately exposes only owner kinds that have a
    /// language-level definition. Procedural owners and lexical scopes remain
    /// owners without a `DefId`.
    ///
    /// Header-shaped owners (module, generate block, block, subroutine) intern
    /// from the owner table only. Their `LocalDefId` is the first row
    /// [`definition_table`] later allocates for that owner, so a subsequent
    /// body-backed lookup yields the same `DefId`. Checker, covergroup, and
    /// clocking still need the lowered body because their origin is an arena
    /// id inside that body.
    pub fn from_owner(db: &dyn HirDefDb, owner: OwnerId) -> Option<Self> {
        match owner.kind(db) {
            OwnerKind::Module
            | OwnerKind::GenerateBlock
            | OwnerKind::Block
            | OwnerKind::Subroutine => Some(Self::from_owner_header(db, owner)),
            OwnerKind::Checker => owner.as_checker(db).map(|origin| Self::from_source(db, origin)),
            OwnerKind::Covergroup => {
                owner.as_covergroup(db).map(|origin| Self::from_source(db, origin))
            }
            OwnerKind::ClockingBlock => {
                owner.as_clocking_block(db).map(|origin| Self::from_source(db, origin))
            }
            OwnerKind::AnonymousProgram | OwnerKind::File | OwnerKind::ProceduralBlock => None,
        }
    }

    fn from_owner_header(db: &dyn HirDefDb, owner: OwnerId) -> Self {
        let loc = match owner.kind(db) {
            OwnerKind::Module => DefOriginLoc::Module(owner),
            OwnerKind::GenerateBlock => DefOriginLoc::GenerateBlock(owner),
            OwnerKind::Block => DefOriginLoc::Block(owner),
            OwnerKind::Subroutine => DefOriginLoc::Subroutine(owner),
            other => {
                unreachable!("header intern is only for named structural owners, got {other:?}")
            }
        };
        let local = LocalDefId(DefinitionKey {
            name: DefinitionNameKey { kind: loc.clone().kind(db), name: loc.name(db) },
            ordinal: 0,
        });
        Self(InternedDefId::new(db, owner, local))
    }

    /// Construct a canonical definition from a typed source representation.
    pub fn from_source(db: &dyn HirDefDb, source: impl Into<DefOriginLoc>) -> Self {
        Self::intern(db, source)
    }

    pub fn from_origin(db: &dyn HirDefDb, origin: DefOrigin) -> Self {
        Self::intern(db, origin.loc(db).clone())
    }

    pub fn origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 3]> {
        let table = db.definition_table(self.0.owner(db));
        let data = table
            .get(self.0.local(db).clone())
            .expect("definition local id must project into its owner table");
        let mut origins = SmallVec::new();
        origins.push(DefOrigin::new(db, data.primary.clone()));
        origins.extend(data.additional.iter().cloned().map(|loc| DefOrigin::new(db, loc)));
        origins
    }

    pub fn primary_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        let table = db.definition_table(self.0.owner(db));
        let data = table
            .get(self.0.local(db).clone())
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

    pub fn name_range(&self, db: &dyn HirDefDb) -> Option<InFile<utils::text_edit::TextRange>> {
        self.primary_origin(db).name_range(db)
    }

    pub fn range(&self, db: &dyn HirDefDb) -> Option<InFile<utils::text_edit::TextRange>> {
        self.primary_origin(db).range(db)
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

    /// Owner in which this definition's declared type is evaluated.
    ///
    /// This keeps declaration-origin details behind the definition seam. It
    /// intentionally mirrors the historical type-container rules: a
    /// subroutine port resolves through its enclosing module, while ordinary
    /// declarations and typedefs use their owning body.
    pub fn type_container(&self, db: &dyn HirDefDb) -> Option<OwnerId> {
        match self.declaration_origin(db).loc(db) {
            DefOriginLoc::Decl(decl) => Some(decl.cont_id),
            DefOriginLoc::Typedef(typedef) => Some(typedef.cont_id),
            DefOriginLoc::SubroutinePort(port) => port.cont_id.parent(db),
            _ => None,
        }
    }

    /// Lowered data type of a declaration-backed definition.
    pub fn data_type(&self, db: &dyn HirDefDb) -> Option<DataTy> {
        match self.declaration_origin(db).loc(db) {
            DefOriginLoc::Typedef(typedef) => {
                typedef.cont_id.data(db).typedef(typedef.value).ty.clone()
            }
            DefOriginLoc::Decl(decl) => {
                let body = decl.cont_id.data(db);
                match body.declarator(decl.value).parent {
                    DeclaratorParent::DeclarationId(parent) => Some(body.declaration(parent).ty()),
                    DeclaratorParent::PortDeclId(port) => Some(body.ports.get(port).header.ty()),
                    DeclaratorParent::StmtId(stmt) => {
                        let StmtKind::For { inits: ForInit::Init(inits), .. } =
                            &body.stmt(stmt).kind
                        else {
                            return None;
                        };
                        inits.iter().find_map(|(ty, candidate)| {
                            (*candidate == decl.value).then(|| ty.clone()).flatten()
                        })
                    }
                }
            }
            DefOriginLoc::SubroutinePort(port) => db
                .subroutine(port.cont_id)
                .ports
                .get(port.value.0 as usize)
                .and_then(|port| port.ty.clone()),
            _ => None,
        }
    }

    /// Presentation classification for editor-facing definition titles.
    pub fn display_label(&self, db: &dyn HirDefDb) -> Option<&'static str> {
        let origin = self.declaration_origin(db);
        match origin.loc(db) {
            DefOriginLoc::Module(owner) => Some(match owner.module_kind(db)? {
                ModuleKind::Module => "Module",
                ModuleKind::Interface => "Interface",
                ModuleKind::Program => "Program",
                ModuleKind::Package => "Package",
            }),
            DefOriginLoc::Config(_) => Some("Config"),
            DefOriginLoc::Library(_) => Some("Library"),
            DefOriginLoc::Udp(_) => Some("Primitive"),
            DefOriginLoc::Block(_) => Some("Block"),
            DefOriginLoc::GenerateBlock(_) => Some("Generate block"),
            DefOriginLoc::Subroutine(owner) => match db.subroutine(*owner).kind {
                SubroutineKind::Task => Some("Task"),
                SubroutineKind::Function { .. } => Some("Function"),
            },
            DefOriginLoc::SubroutinePort(_) | DefOriginLoc::NonAnsiPort(_) => Some("Port"),
            DefOriginLoc::Decl(decl) => {
                let container = decl.cont_id.data(db);
                let declarator = container.declarator(decl.value);
                match declarator.parent {
                    DeclaratorParent::PortDeclId(_) => Some("Port"),
                    DeclaratorParent::StmtId(_) => Some("Variable"),
                    DeclaratorParent::DeclarationId(parent) => {
                        match container.declaration(parent) {
                            Declaration::ParamDecl(param) => (param.kind.keyword() == "localparam")
                                .then_some("Localparam")
                                .or(Some("Parameter")),
                            Declaration::GenvarDecl(_) => Some("Genvar"),
                            Declaration::SpecparamDecl(_) => Some("Specparam"),
                            Declaration::DataDecl(_) | Declaration::NetDecl(_) => {
                                Some("Declaration")
                            }
                        }
                    }
                }
            }
            DefOriginLoc::Typedef(_) => Some("Typedef"),
            DefOriginLoc::Instance(_) => Some("Instance"),
            DefOriginLoc::Modport(_) => Some("Modport"),
            DefOriginLoc::ClockingBlock(_) => Some("Clocking block"),
            DefOriginLoc::Checker(_) => Some("Checker"),
            DefOriginLoc::Covergroup(_) => Some("Covergroup"),
            DefOriginLoc::Property(_) => Some("Property"),
            DefOriginLoc::Sequence(_) => Some("Sequence"),
            DefOriginLoc::Coverpoint(_) => Some("Coverpoint"),
            DefOriginLoc::Cross(_) => Some("Cross"),
            DefOriginLoc::Stmt(_) => Some("Statement"),
            DefOriginLoc::ClockingSignal(_) | DefOriginLoc::CheckerPort(_) => None,
        }
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
        DefOriginLoc::Property(property) => property.cont_id,
        DefOriginLoc::Sequence(sequence) => sequence.cont_id,
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
