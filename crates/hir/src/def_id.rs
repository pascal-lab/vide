use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::{
    ast::AstNode,
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::{
    get::{Get, GetRef},
    line_index::TextRange,
};

use crate::{
    base_db::{intern::Lookup, salsa},
    container::{FileOrModule, InContainer, InFile, InModule, InSubroutine, ScopeId},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::BlockLoc,
        checker::{CheckerDef, CheckerPort, CheckerPortId},
        covergroup::{CoverpointDef, CoverpointId, CrossDef, CrossId},
        declaration::Declaration,
        expr::declarator::DeclaratorParent,
        module::{ModuleKind, clocking::ClockingSignal, generate::GenerateBlockLoc},
        subroutine::{LocalSubroutineId, SubroutineSrc},
    },
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
    symbol::{DefKind, DefOrigin, DefOriginLoc},
};

fn subroutine_src(
    db: &dyn HirDb,
    subroutine: InContainer<LocalSubroutineId>,
) -> Option<InFile<SubroutineSrc>> {
    match subroutine.cont_id {
        ScopeId::File(file_id) => {
            let (_, source_map) = db.hir_file_with_source_map(file_id);
            Some(InFile::new(file_id, source_map.get(subroutine.value)?))
        }
        ScopeId::Module(module_id) => {
            let (_, source_map) = db.module_with_source_map(module_id);
            Some(InFile::new(module_id.file_id, source_map.get(subroutine.value)?))
        }
        ScopeId::GenerateBlock(generate_block_id) => {
            let (_, source_map) = db.generate_block_with_source_map(generate_block_id);
            let file_id = generate_block_id.lookup(db).src.file_id;
            Some(InFile::new(file_id, source_map.get(subroutine.value)?))
        }
        ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => None,
    }
}

fn clocking_signal_of(
    db: &dyn HirDb,
    signal: InContainer<crate::hir_def::module::clocking::ClockingSignalId>,
) -> Option<(InModule<ClockingSignal>, vfs::FileId)> {
    let ScopeId::ClockingBlock(clocking_block) = signal.cont_id else {
        return None;
    };
    let module = db.module(clocking_block.module_id);
    let clocking = module.get(clocking_block.value);
    let signal = clocking.signals.get(signal.value.0 as usize)?.clone();
    Some((InModule::new(clocking_block.module_id, signal), clocking_block.module_id.file_id()))
}

fn checker_of(
    db: &dyn HirDb,
    checker: InContainer<crate::hir_def::checker::CheckerId>,
) -> Option<(CheckerDef, HirFileId)> {
    match checker.cont_id {
        ScopeId::File(file_id) => Some((db.hir_file(file_id).get(checker.value).clone(), file_id)),
        ScopeId::Module(module_id) => {
            Some((db.module(module_id).get(checker.value).clone(), module_id.file_id))
        }
        ScopeId::GenerateBlock(_)
        | ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => None,
    }
}

fn checker_port_of(
    db: &dyn HirDb,
    port: InContainer<CheckerPortId>,
) -> Option<(CheckerPort, HirFileId)> {
    let ScopeId::Checker(checker) = port.cont_id else {
        return None;
    };
    let (checker, file_id) = checker_of(db, checker.as_in_container())?;
    let port = checker.ports.get(port.value.0 as usize)?.clone();
    Some((port, file_id))
}

fn coverpoint_of(
    db: &dyn HirDb,
    coverpoint: InContainer<CoverpointId>,
) -> Option<(CoverpointDef, HirFileId)> {
    let cont_id = match coverpoint.cont_id {
        ScopeId::Covergroup(covergroup) => covergroup.parent_scope(),
        cont_id => cont_id,
    };

    match cont_id {
        ScopeId::File(file_id) => {
            Some((db.hir_file(file_id).get(coverpoint.value).clone(), file_id))
        }
        ScopeId::Module(module_id) => {
            Some((db.module(module_id).get(coverpoint.value).clone(), module_id.file_id))
        }
        ScopeId::GenerateBlock(_)
        | ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => None,
    }
}

fn cross_of(db: &dyn HirDb, cross: InContainer<CrossId>) -> Option<(CrossDef, HirFileId)> {
    let cont_id = match cross.cont_id {
        ScopeId::Covergroup(covergroup) => covergroup.parent_scope(),
        cont_id => cont_id,
    };

    match cont_id {
        ScopeId::File(file_id) => Some((db.hir_file(file_id).get(cross.value).clone(), file_id)),
        ScopeId::Module(module_id) => {
            Some((db.module(module_id).get(cross.value).clone(), module_id.file_id))
        }
        ScopeId::GenerateBlock(_)
        | ScopeId::Block(_)
        | ScopeId::Subroutine(_)
        | ScopeId::ClockingBlock(_)
        | ScopeId::Checker(_)
        | ScopeId::Covergroup(_) => None,
    }
}

impl DefOrigin {
    #[inline]
    pub fn container_id(self, db: &dyn HirDb) -> ScopeId {
        match self.loc(db) {
            DefOriginLoc::Module(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Config(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Library(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Udp(InFile { file_id, .. }) => file_id.into(),
            DefOriginLoc::Block(block_id) => block_id.lookup(db).cont_id,
            DefOriginLoc::GenerateBlock(generate_block_id) => generate_block_id.lookup(db).cont_id,
            DefOriginLoc::Subroutine(subroutine_id) => subroutine_id.cont_id,
            DefOriginLoc::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ScopeId::Subroutine(subroutine.into())
            }
            DefOriginLoc::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::Decl(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Typedef(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Instance(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::Modport(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::ClockingBlock(InModule { module_id, .. }) => module_id.into(),
            DefOriginLoc::ClockingSignal(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Checker(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::CheckerPort(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Covergroup(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Coverpoint(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Cross(InContainer { cont_id, .. }) => cont_id,
            DefOriginLoc::Stmt(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn kind(self, db: &dyn HirDb) -> DefKind {
        match self.loc(db) {
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
                let container = cont_id.to_container(db);
                let decl = container.get(value);
                match decl.parent {
                    DeclaratorParent::PortDeclId(_) => DefKind::Port,
                    DeclaratorParent::StmtId(_) => DefKind::Variable,
                    DeclaratorParent::DeclarationId(declaration_id) => {
                        match container.get(declaration_id) {
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

    pub fn name(self, db: &dyn HirDb) -> Option<SmolStr> {
        match self.loc(db) {
            DefOriginLoc::Module(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::Block(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } = block_id.lookup(db);
                let cont = cont_id.to_container(db);
                value.hir(&cont, &cont_id.to_container_src_map(db))?.name.clone()
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
                cont_id.to_container(db).get(value).name.clone()
            }
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
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
            DefOriginLoc::Checker(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => file_id.to_container(db).get(value).name.clone(),
                ScopeId::Module(module_id) => module_id.to_container(db).get(value).name.clone(),
                ScopeId::GenerateBlock(_)
                | ScopeId::Block(_)
                | ScopeId::Subroutine(_)
                | ScopeId::ClockingBlock(_)
                | ScopeId::Checker(_)
                | ScopeId::Covergroup(_) => None,
            },
            DefOriginLoc::CheckerPort(port) => checker_port_of(db, port).map(|(port, _)| port.name),
            DefOriginLoc::Covergroup(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => file_id.to_container(db).get(value).name.clone(),
                ScopeId::Module(module_id) => module_id.to_container(db).get(value).name.clone(),
                ScopeId::GenerateBlock(_)
                | ScopeId::Block(_)
                | ScopeId::Subroutine(_)
                | ScopeId::ClockingBlock(_)
                | ScopeId::Checker(_)
                | ScopeId::Covergroup(_) => None,
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                coverpoint_of(db, coverpoint).and_then(|(coverpoint, _)| coverpoint.name)
            }
            DefOriginLoc::Cross(cross) => cross_of(db, cross).and_then(|(cross, _)| cross.name),
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).label.clone()
            }
        }
    }

    pub fn name_range(self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        match self.loc(db) {
            DefOriginLoc::Module(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::Block(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
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
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
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
                Some(InFile::new(file_id.into(), signal.value.name_range?))
            }
            DefOriginLoc::Checker(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(file_id, range))
                }
                ScopeId::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(module_id.file_id, range))
                }
                ScopeId::GenerateBlock(_)
                | ScopeId::Block(_)
                | ScopeId::Subroutine(_)
                | ScopeId::ClockingBlock(_)
                | ScopeId::Checker(_)
                | ScopeId::Covergroup(_) => None,
            },
            DefOriginLoc::CheckerPort(port) => {
                let (port, file_id) = checker_port_of(db, port)?;
                Some(InFile::new(file_id, port.name_range?))
            }
            DefOriginLoc::Covergroup(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(file_id, range))
                }
                ScopeId::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(module_id.file_id, range))
                }
                ScopeId::GenerateBlock(_)
                | ScopeId::Block(_)
                | ScopeId::Subroutine(_)
                | ScopeId::ClockingBlock(_)
                | ScopeId::Checker(_)
                | ScopeId::Covergroup(_) => None,
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                let (_, file_id) = coverpoint_of(db, coverpoint)?;
                match coverpoint.cont_id {
                    ScopeId::Covergroup(covergroup) => match covergroup.cont_id {
                        FileOrModule::File(storage_file) => {
                            let range = storage_file
                                .to_container_src_map(db)
                                .get(coverpoint.value)?
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
                    },
                    ScopeId::File(storage_file) => {
                        let range = storage_file
                            .to_container_src_map(db)
                            .get(coverpoint.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                    ScopeId::Module(storage_module) => {
                        let range = storage_module
                            .to_container_src_map(db)
                            .get(coverpoint.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                    _ => None,
                }
            }
            DefOriginLoc::Cross(cross) => {
                let (_, file_id) = cross_of(db, cross)?;
                match cross.cont_id {
                    ScopeId::Covergroup(covergroup) => match covergroup.cont_id {
                        FileOrModule::File(storage_file) => {
                            let range = storage_file
                                .to_container_src_map(db)
                                .get(cross.value)?
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
                    },
                    ScopeId::File(storage_file) => {
                        let range =
                            storage_file.to_container_src_map(db).get(cross.value)?.name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                    ScopeId::Module(storage_module) => {
                        let range = storage_module
                            .to_container_src_map(db)
                            .get(cross.value)?
                            .name_range()?;
                        Some(InFile::new(file_id, range))
                    }
                    _ => None,
                }
            }
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
        }
    }

    pub fn range(self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        Some(match self.loc(db) {
            DefOriginLoc::Module(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::Block(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefOriginLoc::GenerateBlock(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
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
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefOriginLoc::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
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
                InFile::new(file_id.into(), signal.value.name_range?)
            }
            DefOriginLoc::Checker(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(file_id, range)
                }
                ScopeId::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(module_id.file_id, range)
                }
                ScopeId::GenerateBlock(_)
                | ScopeId::Block(_)
                | ScopeId::Subroutine(_)
                | ScopeId::ClockingBlock(_)
                | ScopeId::Checker(_)
                | ScopeId::Covergroup(_) => {
                    return None;
                }
            },
            DefOriginLoc::CheckerPort(port) => {
                let (port, file_id) = checker_port_of(db, port)?;
                InFile::new(file_id, port.name_range?)
            }
            DefOriginLoc::Covergroup(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(file_id, range)
                }
                ScopeId::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(module_id.file_id, range)
                }
                ScopeId::GenerateBlock(_)
                | ScopeId::Block(_)
                | ScopeId::Subroutine(_)
                | ScopeId::ClockingBlock(_)
                | ScopeId::Checker(_)
                | ScopeId::Covergroup(_) => {
                    return None;
                }
            },
            DefOriginLoc::Coverpoint(coverpoint) => {
                let (_, file_id) = coverpoint_of(db, coverpoint)?;
                match coverpoint.cont_id {
                    ScopeId::Covergroup(covergroup) => match covergroup.cont_id {
                        FileOrModule::File(storage_file) => {
                            let range = storage_file
                                .to_container_src_map(db)
                                .get(coverpoint.value)?
                                .range();
                            InFile::new(file_id, range)
                        }
                        FileOrModule::Module(storage_module) => {
                            let range = storage_module
                                .to_container_src_map(db)
                                .get(coverpoint.value)?
                                .range();
                            InFile::new(file_id, range)
                        }
                    },
                    ScopeId::File(storage_file) => {
                        let range =
                            storage_file.to_container_src_map(db).get(coverpoint.value)?.range();
                        InFile::new(file_id, range)
                    }
                    ScopeId::Module(storage_module) => {
                        let range =
                            storage_module.to_container_src_map(db).get(coverpoint.value)?.range();
                        InFile::new(file_id, range)
                    }
                    _ => return None,
                }
            }
            DefOriginLoc::Cross(cross) => {
                let (_, file_id) = cross_of(db, cross)?;
                match cross.cont_id {
                    ScopeId::Covergroup(covergroup) => match covergroup.cont_id {
                        FileOrModule::File(storage_file) => {
                            let range =
                                storage_file.to_container_src_map(db).get(cross.value)?.range();
                            InFile::new(file_id, range)
                        }
                        FileOrModule::Module(storage_module) => {
                            let range =
                                storage_module.to_container_src_map(db).get(cross.value)?.range();
                            InFile::new(file_id, range)
                        }
                    },
                    ScopeId::File(storage_file) => {
                        let range = storage_file.to_container_src_map(db).get(cross.value)?.range();
                        InFile::new(file_id, range)
                    }
                    ScopeId::Module(storage_module) => {
                        let range =
                            storage_module.to_container_src_map(db).get(cross.value)?.range();
                        InFile::new(file_id, range)
                    }
                    _ => return None,
                }
            }
            DefOriginLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Definition {
    primary_origin: DefOrigin,
}

impl Definition {
    fn from_origin(primary_origin: DefOrigin) -> Self {
        Self { primary_origin }
    }

    fn origins(self, db: &dyn HirDb) -> SmallVec<[DefOrigin; 3]> {
        let mut origins = SmallVec::new();
        origins.push(self.primary_origin);
        origins.extend(additional_origins(db, self.primary_origin));
        origins
    }
}

fn additional_origins(db: &dyn HirDb, primary_origin: DefOrigin) -> SmallVec<[DefOrigin; 2]> {
    let Some(port_id) = primary_origin.as_non_ansi_port(db) else {
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
        .map(|(decl_id, _)| DefOrigin::new(db, InContainer::new(port_id.module_id.into(), decl_id)))
        .collect()
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct DefId(pub salsa::InternId);

impl DefId {
    pub fn new(db: &dyn HirDb, loc: impl Into<DefOriginLoc>) -> Self {
        let origin = DefOrigin::new(db, loc);
        let primary_origin = non_ansi_port_for_origin(db, origin)
            .map(|port_id| DefOrigin::new(db, port_id))
            .unwrap_or(origin);
        let definition = Definition::from_origin(primary_origin);
        db.intern_def(definition)
    }

    pub fn origins(self, db: &dyn HirDb) -> SmallVec<[DefOrigin; 3]> {
        db.lookup_intern_def(self).origins(db)
    }

    pub fn primary_origin(self, db: &dyn HirDb) -> DefOrigin {
        db.lookup_intern_def(self).primary_origin
    }

    pub fn declaration_origin(self, db: &dyn HirDb) -> DefOrigin {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port(db).is_some() {
            let additional_origins = additional_origins(db, primary_origin);
            return additional_origins
                .iter()
                .copied()
                .find(|origin| is_port_decl_origin(db, *origin))
                .or_else(|| additional_origins.first().copied())
                .unwrap_or(primary_origin);
        }

        primary_origin
    }

    pub fn def_origins(self, db: &dyn HirDb) -> SmallVec<[DefOrigin; 2]> {
        let primary_origin = self.primary_origin(db);
        if primary_origin.as_non_ansi_port(db).is_some() {
            return additional_origins(db, primary_origin)
                .into_iter()
                .filter(|origin| matches!(origin.loc(db), DefOriginLoc::Decl(_)))
                .collect();
        }

        SmallVec::from_slice(&[primary_origin])
    }

    pub fn is_non_ansi_port(self, db: &dyn HirDb) -> bool {
        self.primary_origin(db).as_non_ansi_port(db).is_some()
    }

    pub fn is_port(self, db: &dyn HirDb) -> bool {
        self.is_non_ansi_port(db)
            || self.origins(db).iter().any(|origin| is_port_decl_origin(db, *origin))
    }

    pub fn container_id(self, db: &dyn HirDb) -> ScopeId {
        self.primary_origin(db).container_id(db)
    }

    pub fn kind(self, db: &dyn HirDb) -> DefKind {
        if self.is_non_ansi_port(db) { DefKind::Port } else { self.primary_origin(db).kind(db) }
    }

    pub fn name(self, db: &dyn HirDb) -> Option<SmolStr> {
        self.primary_origin(db).name(db)
    }
}

fn non_ansi_port_for_origin(
    db: &dyn HirDb,
    origin: DefOrigin,
) -> Option<InModule<crate::hir_def::module::port::NonAnsiPortId>> {
    match origin.loc(db) {
        DefOriginLoc::NonAnsiPort(port_id) => Some(port_id),
        DefOriginLoc::Decl(InContainer { value, cont_id: ScopeId::Module(module_id) }) => {
            let module = db.module(module_id);
            let name = module.get(value).name.as_ref()?;
            let crate::hir_def::module::port::Ports::NonAnsi { ports, .. } = &module.ports else {
                return None;
            };
            ports
                .iter()
                .find(|(_, port)| port.label.as_ref() == Some(name))
                .map(|(port_id, _)| InModule::new(module_id, port_id))
        }
        _ => None,
    }
}

fn is_port_decl_origin(db: &dyn HirDb, origin: DefOrigin) -> bool {
    let DefOriginLoc::Decl(decl_id) = origin.loc(db) else {
        return false;
    };
    matches!(
        decl_id.cont_id.to_container(db).get(decl_id.value).parent,
        DeclaratorParent::PortDeclId(_)
    )
}
