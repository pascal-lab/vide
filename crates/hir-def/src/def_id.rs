use base_db::salsa;
use preproc_expand::file::HirFileId;
use salsa::plumbing::AsId;
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::ast::{self, AstNode};
use utils::{get::GetRef, line_index::TextRange};

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
    let block = body.get(clocking.value);
    let value = block.signals.get(signal.value.0 as usize)?.clone();
    Some((OwnerRef::new(signal.cont_id, value), signal.cont_id.file(db)))
}

fn checker_of(
    db: &dyn HirDefDb,
    checker: OwnerRef<crate::checker::CheckerId>,
) -> Option<(CheckerDef, HirFileId)> {
    let file_id = checker.cont_id.file(db);
    Some((db.body(checker.cont_id).get(checker.value).clone(), file_id))
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
    Some((db.body(coverpoint.cont_id).get(coverpoint.value).clone(), coverpoint.cont_id.file(db)))
}

fn cross_of(db: &dyn HirDefDb, cross: OwnerRef<CrossId>) -> Option<(CrossDef, HirFileId)> {
    Some((db.body(cross.cont_id).get(cross.value).clone(), cross.cont_id.file(db)))
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
            DefOriginLoc::Config(InFile { value, file_id }) => db
                .body(db.owner_table(file_id).file_owner().expect("file owner"))
                .get(value)
                .name
                .clone(),
            DefOriginLoc::Library(InFile { value, file_id }) => db
                .body(db.owner_table(file_id).file_owner().expect("file owner"))
                .get(value)
                .name
                .clone(),
            DefOriginLoc::Udp(InFile { value, file_id }) => db
                .body(db.owner_table(file_id).file_owner().expect("file owner"))
                .get(value)
                .name
                .clone(),
            DefOriginLoc::Block(owner) => owner.name(db),
            DefOriginLoc::GenerateBlock(owner) => db.body(owner).name.clone(),
            DefOriginLoc::Subroutine(owner) => db.subroutine(owner).name.clone(),
            DefOriginLoc::SubroutinePort(OwnerRef { cont_id: subroutine, value }) => {
                db.subroutine(subroutine).ports.get(value.0 as usize)?.name.clone()
            }
            DefOriginLoc::NonAnsiPort(port) => {
                port.cont_id.data(db).ports.get(port.value).label.clone()
            }
            DefOriginLoc::Decl(OwnerRef { value, cont_id }) => {
                cont_id.data(db).declarator(value).name.clone()
            }
            DefOriginLoc::Typedef(OwnerRef { value, cont_id }) => {
                cont_id.data(db).typedef(value).name.clone()
            }
            DefOriginLoc::Instance(OwnerRef { value, cont_id }) => {
                db.body(cont_id).get(value).name.clone()
            }
            DefOriginLoc::Modport(OwnerRef { value, cont_id }) => {
                db.body(cont_id).get(value).name.clone()
            }
            DefOriginLoc::ClockingBlock(OwnerRef { value, cont_id }) => {
                db.body(cont_id).get(value).name.clone()
            }
            DefOriginLoc::ClockingSignal(signal) => {
                clocking_signal_of(db, signal).map(|(signal, _)| signal.value.name)
            }
            DefOriginLoc::Checker(OwnerRef { value, cont_id }) => {
                db.body(cont_id).get(value).name.clone()
            }
            DefOriginLoc::Covergroup(OwnerRef { value, cont_id }) => {
                db.body(cont_id).get(value).name.clone()
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

        fn child_source(
            db: &dyn HirDefDb,
            file_id: HirFileId,
            child: syntax::SyntaxNode<'_>,
            tree: &syntax::SyntaxTree,
        ) -> Option<InFile<SourceAstId>> {
            let source = db.ast_id_map(file_id).id_of_node_in_tree(tree, child)?;
            Some(InFile::new(file_id, source))
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
                let owner = subroutine;
                let file_id = owner.file(db);
                let tree = db.parse(file_id);
                let node = db.ast_id_map(file_id).node(owner.ast_id(db), &tree)?;
                let function = ast::FunctionDeclaration::cast(node)?;
                let port =
                    function.prototype().port_list()?.ports().children().nth(value.0 as usize)?;
                child_source(db, file_id, port.syntax(), &tree)
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
                let checker = port.cont_id.as_checker(db)?;
                let owner = owner_of_checker(db, checker);
                let file_id = owner.file(db);
                let source = owner_source(db, checker.cont_id, checker.value)?;
                let tree = db.parse(file_id);
                let node = db.ast_id_map(file_id).node(source.value, &tree)?;
                let checker = ast::CheckerDeclaration::cast(node)?;
                let port = checker.port_list()?.ports().children().nth(port.value.0 as usize)?;
                child_source(db, file_id, port.syntax(), &tree)
            }
            DefOriginLoc::Coverpoint(point) => {
                owner_source(db, definition_storage_owner(db, point.cont_id), point.value)
            }
            DefOriginLoc::Cross(cross) => {
                owner_source(db, definition_storage_owner(db, cross.cont_id), cross.value)
            }
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
struct InternedDefId {
    #[returns(copy)]
    owner: OwnerId,
    #[returns(copy)]
    source: SourceAstId,
    #[returns(copy)]
    role: DefinitionRole,
}

/// Canonical definition identity: semantic owner plus source AST identity.
/// Arena indices are reconstructed from the current owner store and are never
/// part of equality or hashing.
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
        let origin = DefOrigin::new(db, loc.into());
        let primary_origin = non_ansi_port_for_origin(db, origin)
            .map(|loc| DefOrigin::new(db, DefOriginLoc::NonAnsiPort(loc)))
            .unwrap_or(origin);
        let loc = primary_origin.loc(db);
        let owner = loc.clone().owner(db);
        let source = loc
            .clone()
            .source_ast(db)
            .expect("every semantic definition must have a source AST identity")
            .value;
        Self(InternedDefId::new(db, owner, source, DefinitionRole::of(loc)))
    }

    pub fn origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 3]> {
        let mut origins = SmallVec::new();
        origins.push(self.primary_origin(db));
        origins.extend(additional_origins(db, self.primary_origin(db)));
        origins
    }

    pub fn primary_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        let loc = origin_for_identity(db, self.0.owner(db), self.0.source(db), self.0.role(db))
            .expect("definition identity must project into the current owner store");
        DefOrigin::new(db, loc)
    }

    pub fn declaration_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port(db).is_some() {
            let additional_origins = additional_origins(db, primary_origin);
            return additional_origins
                .iter()
                .find(|origin| is_port_decl_origin(db, **origin))
                .copied()
                .or_else(|| additional_origins.first().copied())
                .unwrap_or(primary_origin);
        }

        primary_origin
    }

    pub fn declaration_origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 2]> {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port(db).is_some() {
            return additional_origins(db, primary_origin)
                .into_iter()
                .filter(|origin| matches!(origin.loc(db), DefOriginLoc::Decl(_)))
                .collect();
        }

        let mut origins = SmallVec::new();
        origins.push(primary_origin);
        origins
    }

    pub fn is_non_ansi_port(&self, db: &dyn HirDefDb) -> bool {
        self.0.role(db) == DefinitionRole::NonAnsiPort
    }

    pub fn is_port(&self, db: &dyn HirDefDb) -> bool {
        self.is_non_ansi_port(db)
            || self.origins(db).iter().any(|origin| is_port_decl_origin(db, *origin))
    }

    pub fn container_id(&self, db: &dyn HirDefDb) -> OwnerId {
        self.primary_origin(db).container_id(db)
    }

    pub fn kind(&self, db: &dyn HirDefDb) -> DefKind {
        if self.is_non_ansi_port(db) { DefKind::Port } else { self.primary_origin(db).kind(db) }
    }

    pub fn name(&self, db: &dyn HirDefDb) -> Option<SmolStr> {
        self.primary_origin(db).name(db)
    }
}

pub(crate) fn definition_owner(db: &dyn HirDefDb, loc: &DefOriginLoc) -> OwnerId {
    match loc {
        DefOriginLoc::Module(owner)
        | DefOriginLoc::Block(owner)
        | DefOriginLoc::GenerateBlock(owner) => *owner,
        DefOriginLoc::Config(item) => file_owner(db, item.file_id),
        DefOriginLoc::Library(item) => file_owner(db, item.file_id),
        DefOriginLoc::Udp(item) => file_owner(db, item.file_id),
        DefOriginLoc::Subroutine(owner) => *owner,
        DefOriginLoc::SubroutinePort(port) => port.cont_id,
        DefOriginLoc::NonAnsiPort(item) => item.cont_id,
        DefOriginLoc::Decl(item) => item.cont_id,
        DefOriginLoc::Typedef(item) => item.cont_id,
        DefOriginLoc::Instance(item) => item.cont_id,
        DefOriginLoc::Modport(item) => item.cont_id,
        DefOriginLoc::Stmt(item) => item.cont_id,
        DefOriginLoc::ClockingBlock(item) => item.cont_id,
        DefOriginLoc::ClockingSignal(item) => item.cont_id,
        DefOriginLoc::Checker(item) => item.cont_id,
        DefOriginLoc::CheckerPort(item) => item.cont_id,
        DefOriginLoc::Covergroup(item) => item.cont_id,
        DefOriginLoc::Coverpoint(item) => item.cont_id,
        DefOriginLoc::Cross(item) => item.cont_id,
    }
}

fn owner_of_checker(_db: &dyn HirDefDb, checker: OwnerRef<crate::checker::CheckerId>) -> OwnerId {
    checker.cont_id
}

fn file_owner(db: &dyn HirDefDb, file_id: HirFileId) -> OwnerId {
    db.owner_table(file_id).file_owner().expect("file must have a canonical owner")
}

fn definition_storage_owner(_db: &dyn HirDefDb, scope: OwnerId) -> OwnerId {
    scope
}

fn origin_for_identity(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
    role: DefinitionRole,
) -> Option<DefOriginLoc> {
    let loc = match role {
        DefinitionRole::Module => DefOriginLoc::Module(owner),
        DefinitionRole::Block => DefOriginLoc::Block(owner),
        DefinitionRole::GenerateBlock => DefOriginLoc::GenerateBlock(owner),
        DefinitionRole::Subroutine => DefOriginLoc::Subroutine(owner),
        DefinitionRole::SubroutinePort => {
            DefOriginLoc::SubroutinePort(subroutine_port_for_source(db, owner, source)?)
        }
        DefinitionRole::ClockingBlock => DefOriginLoc::ClockingBlock(owner.as_clocking_block(db)?),
        DefinitionRole::ClockingSignal => {
            DefOriginLoc::ClockingSignal(clocking_signal_for_source(db, owner, source)?)
        }
        DefinitionRole::Checker => DefOriginLoc::Checker(owner.as_checker(db)?),
        DefinitionRole::CheckerPort => {
            DefOriginLoc::CheckerPort(checker_port_for_source(db, owner, source)?)
        }
        DefinitionRole::Covergroup => DefOriginLoc::Covergroup(owner.as_covergroup(db)?),
        role => {
            let lowered = db.body_with_source_map(owner);
            let sources = lowered.source_map();
            let arena_owner = owner;
            match role {
                DefinitionRole::Config => DefOriginLoc::Config(InFile::new(
                    owner.file(db),
                    sources.config_decl_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Library => DefOriginLoc::Library(InFile::new(
                    owner.file(db),
                    sources.library_decl_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Udp => DefOriginLoc::Udp(InFile::new(
                    owner.file(db),
                    sources.udp_decl_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::NonAnsiPort => {
                    let crate::module::port::PortSrcs::NonAnsi { ports, .. } = &sources.port_srcs
                    else {
                        return None;
                    };
                    DefOriginLoc::NonAnsiPort(OwnerRef::new(owner, ports.src_to_hir(source)?))
                }
                DefinitionRole::Decl => DefOriginLoc::Decl(OwnerRef::new(
                    arena_owner,
                    sources.decl_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Typedef => DefOriginLoc::Typedef(OwnerRef::new(
                    arena_owner,
                    sources.typedef_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Instance => DefOriginLoc::Instance(OwnerRef::new(
                    owner,
                    sources.instance_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Modport => DefOriginLoc::Modport(OwnerRef::new(
                    owner,
                    sources.modport_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Coverpoint => DefOriginLoc::Coverpoint(OwnerRef::new(
                    scope_for_nested_source(db, owner, source)?,
                    sources.coverpoint_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Cross => DefOriginLoc::Cross(OwnerRef::new(
                    scope_for_nested_source(db, owner, source)?,
                    sources.cross_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Stmt => DefOriginLoc::Stmt(OwnerRef::new(
                    arena_owner,
                    sources.stmt_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Module
                | DefinitionRole::Block
                | DefinitionRole::GenerateBlock
                | DefinitionRole::Subroutine
                | DefinitionRole::SubroutinePort
                | DefinitionRole::ClockingBlock
                | DefinitionRole::ClockingSignal
                | DefinitionRole::Checker
                | DefinitionRole::CheckerPort
                | DefinitionRole::Covergroup => unreachable!(),
            }
        }
    };
    Some(loc)
}

fn subroutine_port_for_source(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
) -> Option<OwnerRef<SubroutinePortId>> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    let node = ast_ids.node(owner.ast_id(db), &tree)?;
    let function = ast::FunctionDeclaration::cast(node)?;
    let index = function
        .prototype()
        .port_list()?
        .ports()
        .children()
        .position(|port| ast_ids.id_of_node_in_tree(&tree, port.syntax()) == Some(source))?;
    Some(OwnerRef::new(owner, SubroutinePortId(u32::try_from(index).ok()?)))
}

fn checker_port_for_source(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
) -> Option<OwnerRef<CheckerPortId>> {
    let scope = owner;
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    let node = ast_ids.node(owner.ast_id(db), &tree)?;
    let checker = ast::CheckerDeclaration::cast(node)?;
    let index = checker
        .port_list()?
        .ports()
        .children()
        .position(|port| ast_ids.id_of_node_in_tree(&tree, port.syntax()) == Some(source))?;
    Some(OwnerRef::new(scope, CheckerPortId(u32::try_from(index).ok()?)))
}

fn clocking_signal_for_source(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
) -> Option<OwnerRef<crate::module::clocking::ClockingSignalId>> {
    let scope = owner;
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    let node = ast_ids.node(owner.ast_id(db), &tree)?;
    let clocking = ast::ClockingDeclaration::cast(node)?;
    let mut index = 0usize;
    for item in clocking.items().children() {
        let ast::Member::ClockingItem(item) = item else { continue };
        for decl in item.decls().children() {
            if ast_ids.id_of_node_in_tree(&tree, decl.syntax()) == Some(source) {
                return Some(OwnerRef::new(
                    scope,
                    crate::module::clocking::ClockingSignalId(u32::try_from(index).ok()?),
                ));
            }
            index += 1;
        }
    }
    None
}

fn scope_for_nested_source(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
) -> Option<OwnerId> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    let mut node = Some(ast_ids.node(source, &tree)?);
    while let Some(current) = node {
        let ast_id = ast_ids.id_of_node_in_tree(&tree, current)?;
        if let Some(scope_owner) =
            db.owner_table(file_id).owner_by_ast(ast_id, OwnerKind::Covergroup)
        {
            return Some(scope_owner);
        }
        node = current.parent();
    }
    Some(owner)
}
fn additional_origins(db: &dyn HirDefDb, primary_origin: DefOrigin) -> SmallVec<[DefOrigin; 2]> {
    let Some(port_id) = primary_origin.as_non_ansi_port(db) else {
        return SmallVec::new();
    };
    let crate::module::port::Ports::NonAnsi { bindings, .. } = &port_id.cont_id.data(db).ports
    else {
        return SmallVec::new();
    };
    bindings
        .origins_by_port
        .get(&port_id.value)
        .into_iter()
        .flatten()
        .map(|decl_id| {
            DefOrigin::new(db, DefOriginLoc::Decl(OwnerRef::new(port_id.cont_id, *decl_id)))
        })
        .collect()
}

fn non_ansi_port_for_origin(
    db: &dyn HirDefDb,
    origin: DefOrigin,
) -> Option<OwnerRef<crate::module::port::NonAnsiPortId>> {
    let DefOriginLoc::Decl(OwnerRef { value, cont_id }) = origin.loc(db) else {
        return None;
    };
    let crate::module::port::Ports::NonAnsi { bindings, .. } = &cont_id.data(db).ports else {
        return None;
    };
    bindings.decl_to_port.get(&value).map(|port| OwnerRef::new(*cont_id, *port))
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
