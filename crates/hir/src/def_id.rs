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
    container::{InContainer, InFile, InModule, InSubroutine, ScopeId},
    db::HirDb,
    hir_def::{
        block::BlockLoc,
        declaration::Declaration,
        expr::declarator::DeclaratorParent,
        module::{ModuleKind, generate::GenerateBlockLoc},
        subroutine::{LocalSubroutineId, SubroutineSrc},
    },
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
    symbol::{DefId, DefKind, DefLoc},
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
        ScopeId::Block(_) | ScopeId::Subroutine(_) => None,
    }
}

impl DefId {
    #[inline]
    pub fn container_id(self, db: &dyn HirDb) -> ScopeId {
        match self.loc(db) {
            DefLoc::Module(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Config(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Library(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Udp(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Block(block_id) => block_id.lookup(db).cont_id,
            DefLoc::GenerateBlock(generate_block_id) => generate_block_id.lookup(db).cont_id,
            DefLoc::Subroutine(subroutine_id) => subroutine_id.cont_id,
            DefLoc::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ScopeId::Subroutine(subroutine.into())
            }
            DefLoc::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            DefLoc::Decl(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Typedef(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Instance(InModule { module_id, .. }) => module_id.into(),
            DefLoc::Modport(InModule { module_id, .. }) => module_id.into(),
            DefLoc::ClockingBlock(InModule { module_id, .. }) => module_id.into(),
            DefLoc::Checker(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Covergroup(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Coverpoint(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Cross(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Stmt(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn kind(self, db: &dyn HirDb) -> DefKind {
        match self.loc(db) {
            DefLoc::Module(module_id) => {
                let file = db.hir_file(module_id.file_id);
                match file.get(module_id.value).kind {
                    ModuleKind::Module => DefKind::Module,
                    ModuleKind::Interface => DefKind::Interface,
                    ModuleKind::Program => DefKind::Program,
                    ModuleKind::Package => DefKind::Package,
                }
            }
            DefLoc::Config(_) => DefKind::Config,
            DefLoc::Library(_) => DefKind::Library,
            DefLoc::Udp(_) => DefKind::Udp,
            DefLoc::Block(_) => DefKind::Block,
            DefLoc::GenerateBlock(_) => DefKind::GenerateBlock,
            DefLoc::Subroutine(_) => DefKind::Subroutine,
            DefLoc::SubroutinePort(_) => DefKind::SubroutinePort,
            DefLoc::NonAnsiPort(_) => DefKind::NonAnsiPort,
            DefLoc::Decl(InContainer { value, cont_id }) => {
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
            DefLoc::Typedef(_) => DefKind::Typedef,
            DefLoc::Instance(_) => DefKind::Instance,
            DefLoc::Modport(_) => DefKind::Modport,
            DefLoc::ClockingBlock(_) => DefKind::ClockingBlock,
            DefLoc::Checker(_) => DefKind::Checker,
            DefLoc::Covergroup(_) => DefKind::Covergroup,
            DefLoc::Coverpoint(_) => DefKind::Coverpoint,
            DefLoc::Cross(_) => DefKind::Cross,
            DefLoc::Stmt(_) => DefKind::Stmt,
        }
    }

    pub fn name(self, db: &dyn HirDb) -> Option<SmolStr> {
        match self.loc(db) {
            DefLoc::Module(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Config(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Library(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Udp(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Block(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } = block_id.lookup(db);
                let cont = cont_id.to_container(db);
                value.hir(&cont, &cont_id.to_container_src_map(db))?.name.clone()
            }
            DefLoc::GenerateBlock(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            DefLoc::Subroutine(subroutine_id) => db.subroutine(subroutine_id).name.clone(),
            DefLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                db.subroutine(subroutine).ports.get(value.0 as usize)?.name.clone()
            }
            DefLoc::NonAnsiPort(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).label.clone()
            }
            DefLoc::Decl(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Typedef(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Instance(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Modport(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            DefLoc::ClockingBlock(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            DefLoc::Checker(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => file_id.to_container(db).get(value).name.clone(),
                ScopeId::Module(module_id) => module_id.to_container(db).get(value).name.clone(),
                ScopeId::GenerateBlock(_) | ScopeId::Block(_) | ScopeId::Subroutine(_) => None,
            },
            DefLoc::Covergroup(_) | DefLoc::Coverpoint(_) | DefLoc::Cross(_) => None,
            DefLoc::Stmt(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).label.clone()
            }
        }
    }

    pub fn name_range(self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        match self.loc(db) {
            DefLoc::Module(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefLoc::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefLoc::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefLoc::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefLoc::Block(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefLoc::GenerateBlock(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            DefLoc::Subroutine(subroutine_id) => {
                let src = subroutine_src(db, subroutine_id)?;
                Some(InFile::new(src.file_id, src.value.name_or_full_range()))
            }
            DefLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
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
            DefLoc::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefLoc::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            DefLoc::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            DefLoc::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefLoc::Modport(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefLoc::ClockingBlock(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            DefLoc::Checker(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(file_id, range))
                }
                ScopeId::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                    Some(InFile::new(module_id.file_id, range))
                }
                ScopeId::GenerateBlock(_) | ScopeId::Block(_) | ScopeId::Subroutine(_) => None,
            },
            DefLoc::Covergroup(_) | DefLoc::Coverpoint(_) | DefLoc::Cross(_) => None,
            DefLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
        }
    }

    pub fn range(self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        Some(match self.loc(db) {
            DefLoc::Module(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefLoc::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefLoc::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefLoc::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            DefLoc::Block(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefLoc::GenerateBlock(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            DefLoc::Subroutine(subroutine_id) => {
                let src = subroutine_src(db, subroutine_id)?;
                let range = src.value.range();
                InFile::new(src.file_id, range)
            }
            DefLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
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
            DefLoc::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefLoc::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefLoc::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            DefLoc::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefLoc::Modport(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefLoc::ClockingBlock(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            DefLoc::Checker(InContainer { value, cont_id }) => match cont_id {
                ScopeId::File(file_id) => {
                    let range = file_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(file_id, range)
                }
                ScopeId::Module(module_id) => {
                    let range = module_id.to_container_src_map(db).get(value)?.range();
                    InFile::new(module_id.file_id, range)
                }
                ScopeId::GenerateBlock(_) | ScopeId::Block(_) | ScopeId::Subroutine(_) => {
                    return None;
                }
            },
            DefLoc::Covergroup(_) | DefLoc::Coverpoint(_) | DefLoc::Cross(_) => return None,
            DefLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleDef(pub SmallVec<[DefId; 3]>);

impl ModuleDef {
    pub fn from_def_ids(def_ids: impl IntoIterator<Item = DefId>) -> Option<ModuleDef> {
        let mut def_ids = def_ids.into_iter().collect::<SmallVec<[_; 3]>>();
        def_ids.sort_unstable();
        def_ids.dedup();

        (!def_ids.is_empty()).then_some(ModuleDef(def_ids))
    }

    pub fn def_ids(&self) -> &[DefId] {
        &self.0
    }

    fn into_def_ids(self) -> SmallVec<[DefId; 3]> {
        self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ModuleDefId(pub salsa::InternId);

impl ModuleDefId {
    pub fn origins(self, db: &dyn HirDb) -> SmallVec<[DefId; 3]> {
        db.lookup_intern_module_def(self).into_def_ids()
    }

    pub fn declaration_origin(self, db: &dyn HirDb) -> Option<DefId> {
        let origins = self.origins(db);
        if origins.iter().any(|origin| origin.kind(db) == DefKind::NonAnsiPort) {
            return origins
                .iter()
                .copied()
                .find(|origin| is_port_decl_origin(db, *origin))
                .or_else(|| {
                    origins.iter().copied().find(|origin| matches!(origin.loc(db), DefLoc::Decl(_)))
                });
        }

        origins.first().copied()
    }

    pub fn def_origins(self, db: &dyn HirDb) -> SmallVec<[DefId; 2]> {
        let origins = self.origins(db);
        if origins.iter().any(|origin| origin.kind(db) == DefKind::NonAnsiPort) {
            return origins
                .iter()
                .copied()
                .filter(|origin| matches!(origin.loc(db), DefLoc::Decl(_)))
                .collect();
        }

        origins.first().copied().into_iter().collect()
    }

    pub fn is_port(self, db: &dyn HirDb) -> bool {
        self.origins(db).iter().any(|origin| {
            origin.kind(db) == DefKind::NonAnsiPort || is_port_decl_origin(db, *origin)
        })
    }

    pub fn container_id(self, db: &dyn HirDb) -> Option<ScopeId> {
        let origins = self.origins(db);
        let container_id = origins.first().map(|origin| origin.container_id(db))?;
        debug_assert! {
            origins.iter().all(|origin| origin.container_id(db) == container_id)
        };
        Some(container_id)
    }
}

fn is_port_decl_origin(db: &dyn HirDb, origin: DefId) -> bool {
    let DefLoc::Decl(decl_id) = origin.loc(db) else {
        return false;
    };
    matches!(
        decl_id.cont_id.to_container(db).get(decl_id.value).parent,
        DeclaratorParent::PortDeclId(_)
    )
}
