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
    container::{ContainerId, InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    hir_def::{
        block::BlockLoc,
        expr::declarator::DeclaratorParent,
        module::generate::GenerateBlockLoc,
    },
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
    symbol::{DefId, DefLoc},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleDefOrigin {
    Module(DefId),
    Config(DefId),
    Library(DefId),
    Udp(DefId),
    Block(DefId),
    GenerateBlock(DefId),
    Subroutine(DefId),
    SubroutinePort(DefId),

    NonAnsiPort(DefId),
    Decl(DefId),
    Typedef(DefId),
    Instance(DefId),
    Stmt(DefId),
}

impl ModuleDefOrigin {
    pub fn from_loc(db: &dyn HirDb, loc: impl Into<DefLoc>) -> Self {
        let loc = loc.into();
        let def_id = DefId::new(db, loc);
        Self::from_def_loc(def_id, loc)
    }

    fn from_def_loc(def_id: DefId, loc: DefLoc) -> Self {
        match loc {
            DefLoc::Module(_) => Self::Module(def_id),
            DefLoc::Config(_) => Self::Config(def_id),
            DefLoc::Library(_) => Self::Library(def_id),
            DefLoc::Udp(_) => Self::Udp(def_id),
            DefLoc::Block(_) => Self::Block(def_id),
            DefLoc::GenerateBlock(_) => Self::GenerateBlock(def_id),
            DefLoc::Subroutine(_) => Self::Subroutine(def_id),
            DefLoc::SubroutinePort(_) => Self::SubroutinePort(def_id),
            DefLoc::NonAnsiPort(_) => Self::NonAnsiPort(def_id),
            DefLoc::Decl(_) => Self::Decl(def_id),
            DefLoc::Typedef(_) => Self::Typedef(def_id),
            DefLoc::Instance(_) => Self::Instance(def_id),
            DefLoc::Stmt(_) => Self::Stmt(def_id),
        }
    }

    pub fn def_id(&self) -> DefId {
        match *self {
            Self::Module(def_id)
            | Self::Config(def_id)
            | Self::Library(def_id)
            | Self::Udp(def_id)
            | Self::Block(def_id)
            | Self::GenerateBlock(def_id)
            | Self::Subroutine(def_id)
            | Self::SubroutinePort(def_id)
            | Self::NonAnsiPort(def_id)
            | Self::Decl(def_id)
            | Self::Typedef(def_id)
            | Self::Instance(def_id)
            | Self::Stmt(def_id) => def_id,
        }
    }

    pub fn loc(&self, db: &dyn HirDb) -> DefLoc {
        self.def_id().loc(db)
    }

    #[inline]
    pub fn container_id(&self, db: &dyn HirDb) -> ContainerId {
        match self.loc(db) {
            DefLoc::Module(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Config(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Library(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Udp(InFile { file_id, .. }) => file_id.into(),
            DefLoc::Block(block_id) => block_id.lookup(db).cont_id,
            DefLoc::GenerateBlock(generate_block_id) => generate_block_id.lookup(db).cont_id,
            DefLoc::Subroutine(subroutine_id) => subroutine_id.lookup(db).cont_id.into(),
            DefLoc::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ContainerId::SubroutineId(subroutine)
            }
            DefLoc::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            DefLoc::Decl(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Typedef(InContainer { cont_id, .. }) => cont_id,
            DefLoc::Instance(InModule { module_id, .. }) => module_id.into(),
            DefLoc::Stmt(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn name(&self, db: &dyn HirDb) -> Option<SmolStr> {
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
            DefLoc::Stmt(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).label.clone()
            }
        }
    }

    pub fn name_range(&self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
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
                let src = subroutine_id.lookup(db).src;
                Some(InFile::new(src.file_id, src.value.name_or_full_range()))
            }
            DefLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                let src = subroutine.lookup(db).src;
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
            DefLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
        }
    }

    pub fn range(&self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
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
                let src = subroutine_id.lookup(db).src;
                let range = src.value.range();
                InFile::new(src.file_id, range)
            }
            DefLoc::SubroutinePort(InSubroutine { subroutine, value }) => {
                let src = subroutine.lookup(db).src;
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
            DefLoc::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleDef {
    origins: SmallVec<[ModuleDefOrigin; 3]>,
}

impl ModuleDef {
    pub fn from_origins(origins: impl IntoIterator<Item = ModuleDefOrigin>) -> Option<ModuleDef> {
        let mut origins = origins.into_iter().collect::<SmallVec<[_; 3]>>();
        origins.sort_unstable();
        origins.dedup();

        (!origins.is_empty()).then_some(ModuleDef { origins })
    }

    pub fn origins(&self) -> &[ModuleDefOrigin] {
        &self.origins
    }

    fn into_origins(self) -> SmallVec<[ModuleDefOrigin; 3]> {
        self.origins
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ModuleDefId(pub salsa::InternId);

impl ModuleDefId {
    pub fn origins(self, db: &dyn HirDb) -> SmallVec<[ModuleDefOrigin; 3]> {
        db.lookup_intern_module_def(self).into_origins()
    }

    pub fn declaration_origin(self, db: &dyn HirDb) -> Option<ModuleDefOrigin> {
        let origins = self.origins(db);
        if origins.iter().any(|origin| matches!(origin, ModuleDefOrigin::NonAnsiPort(_))) {
            return origins.iter().copied().find(|origin| is_port_decl_origin(db, origin)).or_else(
                || {
                    origins
                        .iter()
                        .copied()
                        .find(|origin| matches!(origin, ModuleDefOrigin::Decl(_)))
                },
            );
        }

        origins.first().copied()
    }

    pub fn def_origins(self, db: &dyn HirDb) -> SmallVec<[ModuleDefOrigin; 2]> {
        let origins = self.origins(db);
        if origins.iter().any(|origin| matches!(origin, ModuleDefOrigin::NonAnsiPort(_))) {
            return origins
                .iter()
                .copied()
                .filter(|origin| matches!(origin, ModuleDefOrigin::Decl(_)))
                .collect();
        }

        origins.first().copied().into_iter().collect()
    }

    pub fn is_port(self, db: &dyn HirDb) -> bool {
        self.origins(db).iter().any(|origin| match origin {
            ModuleDefOrigin::NonAnsiPort(_) => true,
            ModuleDefOrigin::Decl(_) => is_port_decl_origin(db, origin),
            _ => false,
        })
    }

    pub fn container_id(self, db: &dyn HirDb) -> Option<ContainerId> {
        let origins = self.origins(db);
        let container_id = origins.first().map(|origin| origin.container_id(db))?;
        debug_assert! {
            origins.iter().all(|origin| origin.container_id(db) == container_id)
        };
        Some(container_id)
    }
}

fn is_port_decl_origin(db: &dyn HirDb, origin: &ModuleDefOrigin) -> bool {
    let DefLoc::Decl(decl_id) = origin.loc(db) else {
        return false;
    };
    matches!(
        decl_id.cont_id.to_container(db).get(decl_id.value).parent,
        DeclaratorParent::PortDeclId(_)
    )
}
