use base_db::salsa;
use la_arena::{Idx, RawIdx};
use preproc_expand::file::HirFileId;
use rustc_hash::FxHashMap;
use salsa::plumbing::AsId;
use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::{
    ast::AstNode,
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use triomphe::Arc;
use utils::{
    get::{Get, GetRef},
    line_index::TextRange,
};

use crate::{
    block::BlockLoc,
    checker::{CheckerDef, CheckerPort, CheckerPortId},
    container::{
        ArenaOwnerId, FileOrModule, InContainer, InFile, InFileOrModule, InModule, InScope,
        InSubroutine, ScopeId, SubroutineParent, SubroutineScope,
    },
    covergroup::{CoverpointDef, CoverpointId, CrossDef, CrossId},
    db::HirDefDb,
    declaration::Declaration,
    expr::declarator::DeclaratorParent,
    module::{Module, ModuleKind, clocking::ClockingSignal, generate::GenerateBlockLoc},
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
    subroutine::SubroutineSrc,
    symbol::{DefKind, DefOrigin, DefOriginLoc},
};

pub(crate) fn subroutine_src(
    db: &dyn HirDefDb,
    subroutine: SubroutineScope,
) -> Option<InFile<SubroutineSrc>> {
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
) -> Option<(InModule<ClockingSignal>, HirFileId)> {
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
        | ScopeId::Block(_)
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
            DefOriginLoc::Block(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } =
                    block_id.loc().clone();
                let cont = cont_id.clone().data(db);
                let source_map = cont_id.source_map(db);
                cont.block_info(source_map.block_from_source(value)?).name.clone()
            }
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            DefOriginLoc::Subroutine(subroutine_id) => db.subroutine(subroutine_id).name.clone(),
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

    pub fn name_range(self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        match self {
            DefOriginLoc::Module(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Block(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.loc().clone();
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.loc().clone();
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Subroutine(subroutine_id) => {
                let src = subroutine_src(db, subroutine_id)?;
                Some(InFile::new(src.file_id, src.value.name_or_full_range()))
            }
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                let src = subroutine_src(db, subroutine)?;
                let tree = db.parse(src.file_id);
                let func = src.value.to_node(&tree)?;
                let ports = func
                    .prototype()
                    .port_list()
                    .map(|ports| ports.ports().children().collect::<Vec<_>>())
                    .unwrap_or_default();
                let port = ports
                    .into_iter()
                    .nth(value.0 as usize)
                    .and_then(|port| port.as_function_port())?;
                let declarator = port.declarator();
                let range = declarator.name()?.text_range_in(declarator.syntax())?;
                Some(InFile::new(src.file_id, range))
            }
            DefOriginLoc::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefOriginLoc::Decl(InContainer { value, cont_id }) => {
                let range =
                    cont_id.clone().source_map(db).source_of_declarator(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db), range))
            }
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => {
                let range =
                    cont_id.clone().source_map(db).source_of_typedef(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db), range))
            }
            DefOriginLoc::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefOriginLoc::Modport(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefOriginLoc::ClockingBlock(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefOriginLoc::ClockingSignal(signal) => {
                let (signal, file_id) = clocking_signal_of(db, signal)?;
                Some(InFile::new(file_id, signal.value.name_range?))
            }
            DefOriginLoc::Checker(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => {
                    let range = db.hir_file_with_source_map(file_id).source(value)?.name_range()?;
                    Some(InFile::new(file_id, range))
                }
                FileOrModule::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(module_id.file_id, range))
                }
            },
            DefOriginLoc::CheckerPort(port) => {
                let (port, file_id) = checker_port_of(db, port)?;
                Some(InFile::new(file_id, port.name_range?))
            }
            DefOriginLoc::Covergroup(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => {
                    let range = db.hir_file_with_source_map(file_id).source(value)?.name_range()?;
                    Some(InFile::new(file_id, range))
                }
                FileOrModule::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(module_id.file_id, range))
                }
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                let (_, file_id) = coverpoint_of(db, coverpoint.clone())?;
                match file_or_module_storage(coverpoint.scope_id)? {
                    FileOrModule::File(storage_file) => {
                        let range = db
                            .hir_file_with_source_map(storage_file)
                            .source(coverpoint.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                    FileOrModule::Module(storage_module) => {
                        let range = storage_module
                            .to_container_src_map(db)
                            .get(coverpoint.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                }
            }
            DefOriginLoc::Cross(cross) => {
                let (_, file_id) = cross_of(db, cross.clone())?;
                match file_or_module_storage(cross.scope_id)? {
                    FileOrModule::File(storage_file) => {
                        let range = db
                            .hir_file_with_source_map(storage_file)
                            .source(cross.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                    FileOrModule::Module(storage_module) => {
                        let range = storage_module
                            .to_container_src_map(db)
                            .get(cross.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                }
            }
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.clone().source_map(db).source_of_stmt(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db), range))
            }
        }
    }

    pub fn range(self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        Some(match self {
            DefOriginLoc::Module(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                let range = db.hir_file_with_source_map(file_id).source(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Block(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.loc().clone();
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.loc().clone();
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Subroutine(subroutine_id) => {
                let src = subroutine_src(db, subroutine_id)?;
                let range = src.value.range();
                InFile::new(src.file_id, range)
            }
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                let src = subroutine_src(db, subroutine)?;
                let tree = db.parse(src.file_id);
                let func = src.value.to_node(&tree)?;
                let ports = func.prototype().port_list()?;
                let port = ports
                    .ports()
                    .children()
                    .nth(value.0 as usize)
                    .and_then(|port| port.as_function_port())?;
                let range = port.syntax().text_range()?;
                InFile::new(src.file_id, range)
            }
            DefOriginLoc::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefOriginLoc::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.clone().source_map(db).source_of_declarator(value)?.range();
                InFile::new(cont_id.file_id(db), range)
            }
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.clone().source_map(db).source_of_typedef(value)?.range();
                InFile::new(cont_id.file_id(db), range)
            }
            DefOriginLoc::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefOriginLoc::Modport(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefOriginLoc::ClockingBlock(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefOriginLoc::ClockingSignal(signal) => {
                let (signal, file_id) = clocking_signal_of(db, signal)?;
                InFile::new(file_id, signal.value.name_range?)
            }
            DefOriginLoc::Checker(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => {
                    let range = db.hir_file_with_source_map(file_id).source(value)?.range();
                    InFile::new(file_id, range)
                }
                FileOrModule::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(module_id.file_id, range)
                }
            },
            DefOriginLoc::CheckerPort(port) => {
                let (port, file_id) = checker_port_of(db, port)?;
                InFile::new(file_id, port.name_range?)
            }
            DefOriginLoc::Covergroup(InFileOrModule { value, cont_id }) => match cont_id {
                FileOrModule::File(file_id) => {
                    let range = db.hir_file_with_source_map(file_id).source(value)?.range();
                    InFile::new(file_id, range)
                }
                FileOrModule::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(module_id.file_id, range)
                }
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                let (_, file_id) = coverpoint_of(db, coverpoint.clone())?;
                match file_or_module_storage(coverpoint.scope_id)? {
                    FileOrModule::File(storage_file) => {
                        let range = db
                            .hir_file_with_source_map(storage_file)
                            .source(coverpoint.value)?
                            .range();
                        InFile::new(file_id, range)
                    }
                    FileOrModule::Module(storage_module) => {
                        let range =
                            storage_module.to_container_src_map(db).get(coverpoint.value)?.range();
                        InFile::new(file_id, range)
                    }
                }
            }
            DefOriginLoc::Cross(cross) => {
                let (_, file_id) = cross_of(db, cross.clone())?;
                match file_or_module_storage(cross.scope_id)? {
                    FileOrModule::File(storage_file) => {
                        let range =
                            db.hir_file_with_source_map(storage_file).source(cross.value)?.range();
                        InFile::new(file_id, range)
                    }
                    FileOrModule::Module(storage_module) => {
                        let range =
                            storage_module.to_container_src_map(db).get(cross.value)?.range();
                        InFile::new(file_id, range)
                    }
                }
            }
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.clone().source_map(db).source_of_stmt(value)?.range();
                InFile::new(cont_id.file_id(db), range)
            }
        })
    }
}

impl DefOrigin {
    #[inline]
    pub fn container_id(&self, db: &dyn HirDefDb) -> ScopeId {
        self.loc(db).clone().container_id()
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

#[salsa::interned(unsafe(no_lifetime), revisions = usize::MAX, debug)]
struct InternedDefId {
    #[returns(copy)]
    origin: DefOrigin,
}

/// A definition id, interned so it is `Copy`. The primary origin is the
/// canonical origin of the definition; non-ANSI ports canonicalize to the port
/// label origin so the same logical port always yields the same id.
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
        Self(InternedDefId::new(db, primary_origin))
    }

    pub fn origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 3]> {
        let mut origins = SmallVec::new();
        origins.push(self.primary_origin(db));
        origins.extend(additional_origins(db, self.primary_origin(db)));
        origins
    }

    pub fn primary_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        self.0.origin(db)
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
        self.primary_origin(db).as_non_ansi_port(db).is_some()
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
                DefOriginLoc::Decl(InContainer::new(port_id.module_id.into(), *decl_id)),
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
    for (_, decl) in module.decls.iter() {
        let Some(name) = &decl.name else { continue };
        let Some(role) = non_ansi_port_role(&module, decl.parent) else { continue };
        *role_counts.entry((name.clone(), role)).or_default() += 1;
    }

    let mut index = NonAnsiPortIndex::default();
    for (decl_id, decl) in module.decls.iter() {
        let Some(name) = &decl.name else { continue };
        let Some(role) = non_ansi_port_role(&module, decl.parent) else { continue };
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

fn non_ansi_port_role(module: &Module, parent: DeclaratorParent) -> Option<NonAnsiPortOriginRole> {
    match parent {
        DeclaratorParent::PortDeclId(_) => Some(NonAnsiPortOriginRole::PortDeclaration),
        DeclaratorParent::StmtId(_) => None,
        DeclaratorParent::DeclarationId(declaration_id) => {
            match &module.declarations[declaration_id] {
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
        DefOriginLoc::Decl(InContainer { value, cont_id: ArenaOwnerId::Module(module_id) }) => {
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
