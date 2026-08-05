use preproc_expand::file::HirFileId;
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
    module::{ModuleKind, clocking::ClockingSignal, generate::GenerateBlockLoc},
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

impl DefOrigin {
    #[inline]
    pub fn container_id(&self, _db: &dyn HirDefDb) -> ScopeId {
        match self.loc() {
            DefOriginLoc::Module(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Config(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Library(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Udp(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Block(block_id) => block_id.loc().cont_id.clone().into(),
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                generate_block_id.loc().cont_id.clone().into()
            }
            DefOriginLoc::Subroutine(subroutine_id) => subroutine_id.cont_id.into(),
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ScopeId::Subroutine(subroutine)
            }
            DefOriginLoc::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::Decl(InContainer { cont_id, .. }) => cont_id.into(),
            DefOriginLoc::Typedef(InContainer { cont_id, .. }) => cont_id.into(),
            DefOriginLoc::Instance(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::Modport(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::ClockingBlock(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::ClockingSignal(InScope { scope_id, .. }) => scope_id,
            DefOriginLoc::Checker(InFileOrModule { cont_id, .. }) => cont_id.into(),
            DefOriginLoc::CheckerPort(InScope { scope_id, .. }) => scope_id,
            DefOriginLoc::Covergroup(InFileOrModule { cont_id, .. }) => cont_id.into(),
            DefOriginLoc::Coverpoint(InScope { scope_id, .. }) => scope_id,
            DefOriginLoc::Cross(InScope { scope_id, .. }) => scope_id,
            DefOriginLoc::Stmt(InContainer { cont_id, .. }) => cont_id.into(),
        }
    }

    pub fn kind(&self, db: &dyn HirDefDb) -> DefKind {
        match self.loc() {
            DefOriginLoc::Module(module_id) => {
                let file = db.hir_file(module_id.file_id);
                match file.get(module_id.value).kind {
                    ModuleKind::Module => DefKind::Module,
                    ModuleKind::Interface => DefKind::Interface,
                    ModuleKind::Program => DefKind::Program,
                    ModuleKind::Package => DefKind::Package,
                }
            }
            DefOriginLoc::Config(_) => DefKind::Config,
            DefOriginLoc::Library(_) => DefKind::Library,
            DefOriginLoc::Udp(_) => DefKind::Udp,
            DefOriginLoc::Block(_) => DefKind::Block,
            DefOriginLoc::GenerateBlock(_) => DefKind::GenerateBlock,
            DefOriginLoc::Subroutine(_) => DefKind::Subroutine,
            DefOriginLoc::SubroutinePort(_) => DefKind::SubroutinePort,
            DefOriginLoc::NonAnsiPort(_) => DefKind::NonAnsiPort,
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
            DefOriginLoc::Typedef(_) => DefKind::Typedef,
            DefOriginLoc::Instance(_) => DefKind::Instance,
            DefOriginLoc::Modport(_) => DefKind::Modport,
            DefOriginLoc::ClockingBlock(_) => DefKind::ClockingBlock,
            DefOriginLoc::ClockingSignal(_) => DefKind::ClockingSignal,
            DefOriginLoc::Checker(_) => DefKind::Checker,
            DefOriginLoc::CheckerPort(_) => DefKind::CheckerPort,
            DefOriginLoc::Covergroup(_) => DefKind::Covergroup,
            DefOriginLoc::Coverpoint(_) => DefKind::Coverpoint,
            DefOriginLoc::Cross(_) => DefKind::Cross,
            DefOriginLoc::Stmt(_) => DefKind::Stmt,
        }
    }

    pub fn name(&self, db: &dyn HirDefDb) -> Option<SmolStr> {
        match self.loc() {
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

    pub fn name_range(&self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        match self.loc() {
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

    pub fn range(&self, db: &dyn HirDefDb) -> Option<InFile<TextRange>> {
        Some(match self.loc() {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Definition {
    primary_origin: DefOrigin,
}

impl Definition {
    fn from_origin(primary_origin: DefOrigin) -> Self {
        Self { primary_origin }
    }

    fn origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 3]> {
        let mut origins = SmallVec::new();
        origins.push(self.primary_origin.clone());
        origins.extend(additional_origins(db, self.primary_origin.clone()));
        origins
    }
}

fn additional_origins(db: &dyn HirDefDb, primary_origin: DefOrigin) -> SmallVec<[DefOrigin; 2]> {
    let Some(port_id) = primary_origin.as_non_ansi_port() else {
        return SmallVec::new();
    };
    let module = db.module(port_id.module_id);
    let Some(port_name) = module.get(port_id.value).label.as_ref() else {
        return SmallVec::new();
    };
    module
        .decls
        .iter()
        .filter(|(_, decl)| decl.name.as_ref() == Some(port_name))
        .map(|(decl_id, _)| DefOrigin::new(InContainer::new(port_id.module_id.into(), decl_id)))
        .filter(|origin| non_ansi_port_for_origin(db, origin.clone()) == Some(port_id))
        .collect()
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefId(Arc<Definition>);

impl DefId {
    pub fn new(db: &dyn HirDefDb, loc: impl Into<DefOriginLoc>) -> Self {
        let origin = DefOrigin::new(loc);
        let primary_origin =
            non_ansi_port_for_origin(db, origin.clone()).map(DefOrigin::new).unwrap_or(origin);
        Self(Arc::new(Definition::from_origin(primary_origin)))
    }

    pub fn origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 3]> {
        self.0.origins(db)
    }

    pub fn primary_origin(&self, _db: &dyn HirDefDb) -> DefOrigin {
        self.0.primary_origin.clone()
    }

    pub fn declaration_origin(&self, db: &dyn HirDefDb) -> DefOrigin {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port().is_some() {
            let additional_origins = additional_origins(db, primary_origin.clone());
            return additional_origins
                .iter()
                .find(|origin| is_port_decl_origin(db, (*origin).clone()))
                .cloned()
                .or_else(|| additional_origins.first().cloned())
                .unwrap_or(primary_origin);
        }

        primary_origin
    }

    pub fn declaration_origins(&self, db: &dyn HirDefDb) -> SmallVec<[DefOrigin; 2]> {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port().is_some() {
            return additional_origins(db, primary_origin)
                .into_iter()
                .filter(|origin| matches!(origin.loc(), DefOriginLoc::Decl(_)))
                .collect();
        }

        let mut origins = SmallVec::new();
        origins.push(primary_origin);
        origins
    }

    pub fn is_non_ansi_port(&self, db: &dyn HirDefDb) -> bool {
        self.primary_origin(db).as_non_ansi_port().is_some()
    }

    pub fn is_port(&self, db: &dyn HirDefDb) -> bool {
        self.is_non_ansi_port(db)
            || self.origins(db).iter().any(|origin| is_port_decl_origin(db, origin.clone()))
    }

    pub fn container_id(&self, _db: &dyn HirDefDb) -> ScopeId {
        self.primary_origin(_db).container_id(_db)
    }

    pub fn kind(&self, db: &dyn HirDefDb) -> DefKind {
        if self.is_non_ansi_port(db) { DefKind::Port } else { self.primary_origin(db).kind(db) }
    }

    pub fn name(&self, db: &dyn HirDefDb) -> Option<SmolStr> {
        self.primary_origin(db).name(db)
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum NonAnsiPortOriginRole {
    PortDeclaration,
    DataDeclaration,
}

fn non_ansi_port_origin_role(
    db: &dyn HirDefDb,
    origin: DefOrigin,
) -> Option<NonAnsiPortOriginRole> {
    match origin.kind(db) {
        DefKind::Port => Some(NonAnsiPortOriginRole::PortDeclaration),
        DefKind::Variable | DefKind::Net => Some(NonAnsiPortOriginRole::DataDeclaration),
        _ => None,
    }
}

fn non_ansi_port_for_origin(
    db: &dyn HirDefDb,
    origin: DefOrigin,
) -> Option<InModule<crate::module::port::NonAnsiPortId>> {
    match origin.loc() {
        DefOriginLoc::NonAnsiPort(port_id) => Some(port_id),
        DefOriginLoc::Decl(InContainer { value, cont_id: ArenaOwnerId::Module(module_id) }) => {
            let role = non_ansi_port_origin_role(db, origin.clone())?;
            let module = db.module(module_id);
            let name = module.get(value).name.as_ref()?;
            let matching_role_count = module
                .decls
                .iter()
                .filter(|(_, decl)| decl.name.as_ref() == Some(name))
                .map(|(decl_id, _)| DefOrigin::new(InContainer::new(module_id.into(), decl_id)))
                .filter(|candidate| non_ansi_port_origin_role(db, candidate.clone()) == Some(role))
                .take(2)
                .count();
            if matching_role_count != 1 {
                return None;
            }

            let crate::module::port::Ports::NonAnsi { ports, .. } = &module.ports else {
                return None;
            };
            let mut matches = ports.iter().filter(|(_, port)| port.label.as_ref() == Some(name));
            let (port_id, _) = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            Some(InModule::new(module_id, port_id))
        }
        _ => None,
    }
}

fn is_port_decl_origin(db: &dyn HirDefDb, origin: DefOrigin) -> bool {
    let DefOriginLoc::Decl(decl_id) = origin.loc() else {
        return false;
    };
    matches!(
        decl_id.cont_id.data(db).declarator(decl_id.value).parent,
        DeclaratorParent::PortDeclId(_)
    )
}
