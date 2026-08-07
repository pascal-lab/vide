use base_db::salsa;
use la_arena::{Idx, RawIdx};
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use salsa::plumbing::AsId;
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::ast::{self, AstNode};
use triomphe::Arc;
use utils::{
    get::{Get, GetRef},
    line_index::TextRange,
};

use crate::{
    ast_id_map::SourceAstId,
    checker::{CheckerDef, CheckerPort, CheckerPortId},
    container::{
        FileOrModule, InContainer, InFile, InFileOrModule, InModule, InScope, InSubroutine,
        ScopeId, SubroutineParent, SubroutineScope,
    },
    covergroup::{CoverpointDef, CoverpointId, CrossDef, CrossId},
    db::HirDefDb,
    declaration::Declaration,
    expr::declarator::DeclaratorParent,
    has_source::HasSource,
    module::ModuleKind,
    owner::{OwnerId, OwnerKind},
    subroutine::SubroutinePortId,
    symbol::{DefKind, DefOrigin, DefOriginLoc},
};

pub(crate) fn subroutine_src(
    db: &dyn HirDefDb,
    subroutine: SubroutineScope,
) -> Option<InFile<SourceAstId>> {
    match subroutine.cont_id {
        SubroutineParent::File(file_id) => {
            let lowered = db.hir_file_with_source_map(file_id);
            Some(InFile::new(file_id, lowered.source(subroutine.value)?))
        }
        SubroutineParent::Module(module_id) => {
            let lowered = db.module_with_source_map(module_id);
            Some(InFile::new(module_id.file_id, lowered.source(subroutine.value)?))
        }
        SubroutineParent::GenerateBlock(generate_block_id) => {
            let lowered = db.generate_block_with_source_map(generate_block_id.clone());
            let file_id = generate_block_id.loc().src.file_id;
            Some(InFile::new(file_id, lowered.source(subroutine.value)?))
        }
    }
}

fn clocking_signal_of(
    db: &dyn HirDefDb,
    signal: InScope<crate::module::clocking::ClockingSignalId>,
) -> Option<(InModule<crate::module::clocking::ClockingSignal>, HirFileId)> {
    let ScopeId::ClockingBlock(clocking_block) = signal.scope_id else {
        return None;
    };
    let module = db.module(clocking_block.module_id);
    let clocking = module.get(clocking_block.value);
    let signal = clocking.signals.get(signal.value.0 as usize)?.clone();
    Some((InModule::new(clocking_block.module_id, signal), clocking_block.module_id.file_id))
}

fn checker_of(
    db: &dyn HirDefDb,
    checker: InFileOrModule<crate::checker::CheckerId>,
) -> Option<(CheckerDef, HirFileId)> {
    match checker.cont_id {
        FileOrModule::File(file_id) => {
            Some((db.hir_file(file_id).get(checker.value).clone(), file_id))
        }
        FileOrModule::Module(module_id) => {
            Some((db.module(module_id).get(checker.value).clone(), module_id.file_id))
        }
    }
}

fn checker_port_of(
    db: &dyn HirDefDb,
    port: InScope<CheckerPortId>,
) -> Option<(CheckerPort, HirFileId)> {
    let ScopeId::Checker(checker) = port.scope_id else {
        return None;
    };
    let (checker, file_id) = checker_of(db, checker)?;
    let port = checker.ports.get(port.value.0 as usize)?.clone();
    Some((port, file_id))
}

fn file_or_module_storage(scope_id: ScopeId) -> Option<FileOrModule> {
    match scope_id {
        ScopeId::Covergroup(covergroup) => Some(covergroup.cont_id),
        ScopeId::File(file_id) => Some(FileOrModule::File(file_id)),
        ScopeId::Module(module_id) => Some(FileOrModule::Module(module_id)),
        ScopeId::GenerateBlock(_)
        | ScopeId::Subroutine(_)
        | ScopeId::Owner(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_) => None,
    }
}

fn coverpoint_of(
    db: &dyn HirDefDb,
    coverpoint: InScope<CoverpointId>,
) -> Option<(CoverpointDef, HirFileId)> {
    let cont_id = file_or_module_storage(coverpoint.scope_id)?;

    match cont_id {
        FileOrModule::File(file_id) => {
            Some((db.hir_file(file_id).get(coverpoint.value).clone(), file_id))
        }
        FileOrModule::Module(module_id) => {
            Some((db.module(module_id).get(coverpoint.value).clone(), module_id.file_id))
        }
    }
}

fn cross_of(db: &dyn HirDefDb, cross: InScope<CrossId>) -> Option<(CrossDef, HirFileId)> {
    let cont_id = file_or_module_storage(cross.scope_id)?;

    match cont_id {
        FileOrModule::File(file_id) => {
            Some((db.hir_file(file_id).get(cross.value).clone(), file_id))
        }
        FileOrModule::Module(module_id) => {
            Some((db.module(module_id).get(cross.value).clone(), module_id.file_id))
        }
    }
}

impl DefOriginLoc {
    pub fn kind(self, db: &dyn HirDefDb) -> DefKind {
        match self {
            DefOriginLoc::Module(module_id) => {
                let file = db.hir_file(module_id.file_id);
                match file.get(module_id.value).kind {
                    ModuleKind::Module => DefKind::Module,
                    ModuleKind::Interface => DefKind::Interface,
                    ModuleKind::Program => DefKind::Program,
                    ModuleKind::Package => DefKind::Package,
                }
            }
            DefOriginLoc::Decl(InContainer { value, cont_id }) => {
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
            DefOriginLoc::Module(InFile { value, file_id }) => {
                db.hir_file(file_id).get(value).name.clone()
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                db.hir_file(file_id).get(value).name.clone()
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                db.hir_file(file_id).get(value).name.clone()
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                db.hir_file(file_id).get(value).name.clone()
            }
            DefOriginLoc::Block(owner) => owner.name(db),
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            DefOriginLoc::Subroutine(subroutine_id) => subroutine_id
                .clone()
                .owner(db)
                .and_then(|owner| db.item_for_owner(owner))
                .and_then(|item| item.name().cloned()),
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                db.subroutine(subroutine).ports.get(value.0 as usize)?.name.clone()
            }
            DefOriginLoc::NonAnsiPort(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).label.clone()
            }
            DefOriginLoc::Decl(InContainer { value, cont_id }) => {
                cont_id.data(db).declarator(value).name.clone()
            }
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => {
                cont_id.data(db).typedef(value).name.clone()
            }
            DefOriginLoc::Instance(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::Modport(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::ClockingBlock(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::ClockingSignal(signal) => {
                clocking_signal_of(db, signal).map(|(signal, _)| signal.value.name)
            }
            DefOriginLoc::Checker(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => db.hir_file(file_id).get(value).name.clone(),
                FileOrModule::Module(module_id) => {
                    module_id.to_container(db).get(value).name.clone()
                }
            },
            DefOriginLoc::CheckerPort(port) => checker_port_of(db, port).map(|(port, _)| port.name),
            DefOriginLoc::Covergroup(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => db.hir_file(file_id).get(value).name.clone(),
                FileOrModule::Module(module_id) => {
                    module_id.to_container(db).get(value).name.clone()
                }
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                coverpoint_of(db, coverpoint.clone()).and_then(|(coverpoint, _)| coverpoint.name)
            }
            DefOriginLoc::Cross(cross) => {
                cross_of(db, cross.clone()).and_then(|(cross, _)| cross.name)
            }
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => {
                cont_id.data(db).stmt(value).label.clone()
            }
        }
    }

    pub(crate) fn source_ast(self, db: &dyn HirDefDb) -> Option<InFile<SourceAstId>> {
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
            DefOriginLoc::Module(InFile { value, file_id }) => {
                Some(InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                Some(InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                Some(InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                Some(InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
            }
            DefOriginLoc::Block(owner) => Some(InFile::new(owner.file(db), owner.ast_id(db))),
            DefOriginLoc::GenerateBlock(generate_block) => Some(generate_block.loc().src),
            DefOriginLoc::Subroutine(subroutine) => subroutine_src(db, subroutine),
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                let source = subroutine_src(db, subroutine)?;
                let tree = db.parse(source.file_id);
                let node = db.ast_id_map(source.file_id).node(source.value, &tree)?;
                let function = ast::FunctionDeclaration::cast(node)?;
                let port =
                    function.prototype().port_list()?.ports().children().nth(value.0 as usize)?;
                child_source(db, source.file_id, port.syntax(), &tree)
            }
            DefOriginLoc::NonAnsiPort(InModule { value, module_id }) => {
                Some(InFile::new(module_id.file_id, module_id.to_container_src_map(db).get(value)?))
            }
            DefOriginLoc::Decl(InContainer { value, cont_id }) => Some(InFile::new(
                cont_id.file_id(db),
                cont_id.clone().source_map(db).source_of_declarator(value)?,
            )),
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => Some(InFile::new(
                cont_id.file_id(db),
                cont_id.clone().source_map(db).source_of_typedef(value)?,
            )),
            DefOriginLoc::Instance(InModule { value, module_id }) => {
                Some(InFile::new(module_id.file_id, module_id.to_container_src_map(db).get(value)?))
            }
            DefOriginLoc::Modport(InModule { value, module_id }) => {
                Some(InFile::new(module_id.file_id, module_id.to_container_src_map(db).get(value)?))
            }
            DefOriginLoc::ClockingBlock(InModule { value, module_id }) => {
                Some(InFile::new(module_id.file_id, module_id.to_container_src_map(db).get(value)?))
            }
            DefOriginLoc::ClockingSignal(signal) => {
                let ScopeId::ClockingBlock(clocking) = signal.scope_id else { return None };
                let file_id = clocking.module_id.file_id;
                let source = clocking.module_id.to_container_src_map(db).get(clocking.value)?;
                let tree = db.parse(file_id);
                let node = db.ast_id_map(file_id).node(source, &tree)?;
                let clocking = ast::ClockingDeclaration::cast(node)?;
                let decl = clocking
                    .items()
                    .children()
                    .filter_map(|item| match item {
                        ast::Member::ClockingItem(item) => {
                            Some(item.decls().children().collect::<Vec<_>>())
                        }
                        _ => None,
                    })
                    .flatten()
                    .nth(signal.value.0 as usize)?;
                child_source(db, file_id, decl.syntax(), &tree)
            }
            DefOriginLoc::Checker(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => {
                    Some(InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
                }
                FileOrModule::Module(module_id) => Some(InFile::new(
                    module_id.file_id,
                    module_id.to_container_src_map(db).get(value)?,
                )),
            },
            DefOriginLoc::CheckerPort(port) => {
                let ScopeId::Checker(checker) = port.scope_id else { return None };
                let (file_id, source) = match checker.cont_id {
                    FileOrModule::File(file_id) => {
                        (file_id, db.hir_file_with_source_map(file_id).source(checker.value)?)
                    }
                    FileOrModule::Module(module_id) => {
                        (module_id.file_id, module_id.to_container_src_map(db).get(checker.value)?)
                    }
                };
                let tree = db.parse(file_id);
                let node = db.ast_id_map(file_id).node(source, &tree)?;
                let checker = ast::CheckerDeclaration::cast(node)?;
                let port = checker.port_list()?.ports().children().nth(port.value.0 as usize)?;
                child_source(db, file_id, port.syntax(), &tree)
            }
            DefOriginLoc::Covergroup(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => {
                    Some(InFile::new(file_id, db.hir_file_with_source_map(file_id).source(value)?))
                }
                FileOrModule::Module(module_id) => Some(InFile::new(
                    module_id.file_id,
                    module_id.to_container_src_map(db).get(value)?,
                )),
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                match file_or_module_storage(coverpoint.scope_id)? {
                    FileOrModule::File(file_id) => Some(InFile::new(
                        file_id,
                        db.hir_file_with_source_map(file_id).source(coverpoint.value)?,
                    )),
                    FileOrModule::Module(module_id) => Some(InFile::new(
                        module_id.file_id,
                        module_id.to_container_src_map(db).get(coverpoint.value)?,
                    )),
                }
            }
            DefOriginLoc::Cross(cross) => match file_or_module_storage(cross.scope_id)? {
                FileOrModule::File(file_id) => Some(InFile::new(
                    file_id,
                    db.hir_file_with_source_map(file_id).source(cross.value)?,
                )),
                FileOrModule::Module(module_id) => Some(InFile::new(
                    module_id.file_id,
                    module_id.to_container_src_map(db).get(cross.value)?,
                )),
            },
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => Some(InFile::new(
                cont_id.file_id(db),
                cont_id.clone().source_map(db).source_of_stmt(value)?,
            )),
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
    pub fn container_id(&self, db: &dyn HirDefDb) -> ScopeId {
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
        let owner = definition_owner(db, loc);
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

    pub fn container_id(&self, db: &dyn HirDefDb) -> ScopeId {
        self.primary_origin(db).container_id(db)
    }

    pub fn kind(&self, db: &dyn HirDefDb) -> DefKind {
        if self.is_non_ansi_port(db) { DefKind::Port } else { self.primary_origin(db).kind(db) }
    }

    pub fn name(&self, db: &dyn HirDefDb) -> Option<SmolStr> {
        self.primary_origin(db).name(db)
    }
}

fn definition_owner(db: &dyn HirDefDb, loc: &DefOriginLoc) -> OwnerId {
    match loc {
        DefOriginLoc::Module(module) => module.owner(db).expect("module definition owner"),
        DefOriginLoc::Config(item) => file_owner(db, item.file_id),
        DefOriginLoc::Library(item) => file_owner(db, item.file_id),
        DefOriginLoc::Udp(item) => file_owner(db, item.file_id),
        DefOriginLoc::Block(owner) => *owner,
        DefOriginLoc::GenerateBlock(block) => {
            block.clone().owner(db).expect("generate block definition owner")
        }
        DefOriginLoc::Subroutine(subroutine) => {
            subroutine.clone().owner(db).expect("subroutine definition owner")
        }
        DefOriginLoc::SubroutinePort(port) => {
            port.subroutine.clone().owner(db).expect("subroutine port owner")
        }
        DefOriginLoc::NonAnsiPort(port) => port.module_id.owner(db).expect("module port owner"),
        DefOriginLoc::Decl(item) => item.cont_id,
        DefOriginLoc::Typedef(item) => item.cont_id,
        DefOriginLoc::Instance(item) => item.module_id.owner(db).expect("instance owner"),
        DefOriginLoc::Modport(item) => item.module_id.owner(db).expect("modport owner"),
        DefOriginLoc::ClockingBlock(item) => ScopeId::ClockingBlock(*item).owner(db),
        DefOriginLoc::ClockingSignal(item) => item.scope_id.owner(db),
        DefOriginLoc::Checker(item) => ScopeId::Checker(*item).owner(db),
        DefOriginLoc::CheckerPort(item) => item.scope_id.owner(db),
        DefOriginLoc::Covergroup(item) => ScopeId::Covergroup(*item).owner(db),
        DefOriginLoc::Coverpoint(item) => definition_storage_owner(db, &item.scope_id),
        DefOriginLoc::Cross(item) => definition_storage_owner(db, &item.scope_id),
        DefOriginLoc::Stmt(item) => item.cont_id,
    }
}

fn file_owner(db: &dyn HirDefDb, file_id: HirFileId) -> OwnerId {
    db.owner_table(file_id).file_owner().expect("file must have a canonical owner")
}

fn definition_storage_owner(db: &dyn HirDefDb, scope: &ScopeId) -> OwnerId {
    match scope {
        ScopeId::Covergroup(covergroup) => covergroup.parent_scope().owner(db),
        _ => scope.owner(db),
    }
}

fn origin_for_identity(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
    role: DefinitionRole,
) -> Option<DefOriginLoc> {
    let loc = match role {
        DefinitionRole::Module => {
            DefOriginLoc::Module(crate::module::ModuleId::from_owner(db, owner)?)
        }
        DefinitionRole::Block => DefOriginLoc::Block(owner),
        DefinitionRole::GenerateBlock => DefOriginLoc::GenerateBlock(
            crate::module::generate::GenerateBlockId::from_owner(db, owner)?,
        ),
        DefinitionRole::Subroutine => {
            DefOriginLoc::Subroutine(SubroutineScope::from_owner(db, owner)?)
        }
        DefinitionRole::SubroutinePort => {
            DefOriginLoc::SubroutinePort(subroutine_port_for_source(db, owner, source)?)
        }
        DefinitionRole::ClockingBlock => match ScopeId::from_owner(db, owner)? {
            ScopeId::ClockingBlock(item) => DefOriginLoc::ClockingBlock(item),
            _ => return None,
        },
        DefinitionRole::ClockingSignal => {
            DefOriginLoc::ClockingSignal(clocking_signal_for_source(db, owner, source)?)
        }
        DefinitionRole::Checker => match ScopeId::from_owner(db, owner)? {
            ScopeId::Checker(item) => DefOriginLoc::Checker(item),
            _ => return None,
        },
        DefinitionRole::CheckerPort => {
            DefOriginLoc::CheckerPort(checker_port_for_source(db, owner, source)?)
        }
        DefinitionRole::Covergroup => match ScopeId::from_owner(db, owner)? {
            ScopeId::Covergroup(item) => DefOriginLoc::Covergroup(item),
            _ => return None,
        },
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
                    let module = crate::module::ModuleId::from_owner(db, owner)?;
                    let crate::module::port::PortSrcs::NonAnsi { ports, .. } = &sources.port_srcs
                    else {
                        return None;
                    };
                    DefOriginLoc::NonAnsiPort(InModule::new(module, ports.src_to_hir(source)?))
                }
                DefinitionRole::Decl => DefOriginLoc::Decl(InContainer::new(
                    arena_owner,
                    sources.decl_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Typedef => DefOriginLoc::Typedef(InContainer::new(
                    arena_owner,
                    sources.typedef_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Instance => {
                    let module = crate::module::ModuleId::from_owner(db, owner)?;
                    DefOriginLoc::Instance(InModule::new(
                        module,
                        sources.instance_srcs.src_to_hir(source)?,
                    ))
                }
                DefinitionRole::Modport => {
                    let module = crate::module::ModuleId::from_owner(db, owner)?;
                    DefOriginLoc::Modport(InModule::new(
                        module,
                        sources.modport_srcs.src_to_hir(source)?,
                    ))
                }
                DefinitionRole::Coverpoint => DefOriginLoc::Coverpoint(InScope::new(
                    scope_for_nested_source(db, owner, source)?,
                    sources.coverpoint_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Cross => DefOriginLoc::Cross(InScope::new(
                    scope_for_nested_source(db, owner, source)?,
                    sources.cross_srcs.src_to_hir(source)?,
                )),
                DefinitionRole::Stmt => DefOriginLoc::Stmt(InContainer::new(
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
) -> Option<InSubroutine<SubroutinePortId>> {
    let subroutine = SubroutineScope::from_owner(db, owner)?;
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
    Some(InSubroutine::new(subroutine, SubroutinePortId(u32::try_from(index).ok()?)))
}

fn checker_port_for_source(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
) -> Option<InScope<CheckerPortId>> {
    let scope = ScopeId::from_owner(db, owner)?;
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
    Some(InScope::new(scope, CheckerPortId(u32::try_from(index).ok()?)))
}

fn clocking_signal_for_source(
    db: &dyn HirDefDb,
    owner: OwnerId,
    source: SourceAstId,
) -> Option<InScope<crate::module::clocking::ClockingSignalId>> {
    let scope = ScopeId::from_owner(db, owner)?;
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
                return Some(InScope::new(
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
) -> Option<ScopeId> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let ast_ids = db.ast_id_map(file_id);
    let mut node = Some(ast_ids.node(source, &tree)?);
    while let Some(current) = node {
        let ast_id = ast_ids.id_of_node_in_tree(&tree, current)?;
        if let Some(scope_owner) =
            db.owner_table(file_id).owner_by_ast(ast_id, OwnerKind::Covergroup)
        {
            return ScopeId::from_owner(db, scope_owner);
        }
        node = current.parent();
    }
    Some(match owner.kind(db) {
        OwnerKind::File => ScopeId::File(file_id),
        OwnerKind::Module => ScopeId::Module(crate::module::ModuleId::from_owner(db, owner)?),
        _ => ScopeId::Owner(owner),
    })
}
fn additional_origins(db: &dyn HirDefDb, primary_origin: DefOrigin) -> SmallVec<[DefOrigin; 2]> {
    let Some(port_id) = primary_origin.as_non_ansi_port(db) else {
        return SmallVec::new();
    };
    let index = non_ansi_port_index(
        db,
        port_id.module_id.file_id,
        u32::from(port_id.module_id.value.into_raw()),
    );
    index
        .origins_by_port
        .get(&port_id.value)
        .into_iter()
        .flatten()
        .map(|decl_id| {
            DefOrigin::new(
                db,
                DefOriginLoc::Decl(InContainer::new(
                    port_id.module_id.owner(db).expect("module owner"),
                    *decl_id,
                )),
            )
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum NonAnsiPortOriginRole {
    PortDeclaration,
    DataDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct NonAnsiPortIndex {
    declaration_to_port:
        FxHashMap<crate::expr::declarator::DeclId, InModule<crate::module::port::NonAnsiPortId>>,
    origins_by_port: FxHashMap<
        crate::module::port::NonAnsiPortId,
        SmallVec<[crate::expr::declarator::DeclId; 2]>,
    >,
}

#[salsa::tracked(lru = 128, returns(clone))]
fn non_ansi_port_index(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    module_index: u32,
) -> Arc<NonAnsiPortIndex> {
    let module_id =
        crate::module::ModuleId::new(file_id, Idx::from_raw(RawIdx::from(module_index)));
    let module = db.module(module_id);
    let owner = module_id.owner(db).expect("module must have a canonical owner");
    let body = db.body_with_source_map(owner);
    let crate::module::port::Ports::NonAnsi { ports, .. } = &module.ports else {
        return Arc::new(NonAnsiPortIndex::default());
    };

    let mut ports_by_name =
        FxHashMap::<SmolStr, SmallVec<[crate::module::port::NonAnsiPortId; 2]>>::default();
    for (port_id, port) in ports.iter() {
        if let Some(name) = &port.label {
            ports_by_name.entry(name.clone()).or_default().push(port_id);
        }
    }

    let mut role_counts = FxHashMap::<(SmolStr, NonAnsiPortOriginRole), usize>::default();
    for (_, decl) in body.decls.iter() {
        let Some(name) = &decl.name else { continue };
        let Some(role) = non_ansi_port_role(&body, decl.parent) else { continue };
        *role_counts.entry((name.clone(), role)).or_default() += 1;
    }

    let mut index = NonAnsiPortIndex::default();
    for (decl_id, decl) in body.decls.iter() {
        let Some(name) = &decl.name else { continue };
        let Some(role) = non_ansi_port_role(&body, decl.parent) else { continue };
        if role_counts.get(&(name.clone(), role)) != Some(&1) {
            continue;
        }
        let Some(port_ids) = ports_by_name.get(name) else { continue };
        let [port_id] = port_ids.as_slice() else { continue };
        let port_id = InModule::new(module_id, *port_id);
        index.declaration_to_port.insert(decl_id, port_id);
        index.origins_by_port.entry(port_id.value).or_default().push(decl_id);
    }

    Arc::new(index)
}

fn non_ansi_port_role(
    body: &crate::body::Body,
    parent: DeclaratorParent,
) -> Option<NonAnsiPortOriginRole> {
    match parent {
        DeclaratorParent::PortDeclId(_) => Some(NonAnsiPortOriginRole::PortDeclaration),
        DeclaratorParent::StmtId(_) => None,
        DeclaratorParent::DeclarationId(declaration_id) => {
            match &body.declarations[declaration_id] {
                Declaration::DataDecl(_) | Declaration::NetDecl(_) => {
                    Some(NonAnsiPortOriginRole::DataDeclaration)
                }
                Declaration::ParamDecl(_)
                | Declaration::GenvarDecl(_)
                | Declaration::SpecparamDecl(_) => None,
            }
        }
    }
}

fn non_ansi_port_for_origin(
    db: &dyn HirDefDb,
    origin: DefOrigin,
) -> Option<InModule<crate::module::port::NonAnsiPortId>> {
    match origin.loc(db) {
        DefOriginLoc::NonAnsiPort(port_id) => Some(port_id.clone()),
        DefOriginLoc::Decl(InContainer { value, cont_id }) => {
            let module_id = crate::module::ModuleId::from_owner(db, *cont_id)?;
            non_ansi_port_index(db, module_id.file_id, u32::from(module_id.value.into_raw()))
                .declaration_to_port
                .get(value)
                .copied()
        }
        _ => None,
    }
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
