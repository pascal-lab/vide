use smallvec::SmallVec;
use smol_str::SmolStr;
use syntax::{
    ast::AstNode,
    has_text_range::{HasTextRange, HasTextRangeIn},
};
use utils::{
    get::{Get, GetRef},
    impl_from,
    line_index::TextRange,
};

use crate::{
    base_db::{intern::Lookup, salsa},
    container::{ContainerId, InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        expr::declarator::{DeclId, DeclaratorParent},
        file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
        module::{
            ModuleId,
            generate::{GenerateBlockId, GenerateBlockLoc},
            instantiation::InstanceId,
            port::NonAnsiPortId,
        },
        stmt::StmtId,
        subroutine::{SubroutineId, SubroutinePortId},
        typedef::TypedefId,
    },
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleDefOrigin {
    ModuleId(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    BlockId(BlockId),
    GenerateBlockId(GenerateBlockId),
    SubroutineId(SubroutineId),
    SubroutinePort(InSubroutine<SubroutinePortId>),

    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl_from! { ModuleDefOrigin =>
    ModuleId,
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    BlockId,
    GenerateBlockId,
    SubroutineId,
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort(InModule<NonAnsiPortId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
}

impl ModuleDefOrigin {
    #[inline]
    pub fn container_id(&self, db: &dyn HirDb) -> ContainerId {
        match *self {
            ModuleDefOrigin::ModuleId(InFile { file_id, .. }) => file_id.into(),
            ModuleDefOrigin::Config(InFile { file_id, .. }) => file_id.into(),
            ModuleDefOrigin::Library(InFile { file_id, .. }) => file_id.into(),
            ModuleDefOrigin::Udp(InFile { file_id, .. }) => file_id.into(),
            ModuleDefOrigin::BlockId(block_id) => block_id.lookup(db).cont_id,
            ModuleDefOrigin::GenerateBlockId(generate_block_id) => {
                generate_block_id.lookup(db).cont_id
            }
            ModuleDefOrigin::SubroutineId(subroutine_id) => subroutine_id.lookup(db).cont_id.into(),
            ModuleDefOrigin::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ContainerId::SubroutineId(subroutine)
            }
            ModuleDefOrigin::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            ModuleDefOrigin::Decl(InContainer { cont_id, .. }) => cont_id,
            ModuleDefOrigin::Typedef(InContainer { cont_id, .. }) => cont_id,
            ModuleDefOrigin::Instance(InModule { module_id, .. }) => module_id.into(),
            ModuleDefOrigin::Stmt(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn name(&self, db: &dyn HirDb) -> Option<SmolStr> {
        match *self {
            ModuleDefOrigin::ModuleId(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::Config(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::Library(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::Udp(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::BlockId(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } = block_id.lookup(db);
                let cont = cont_id.to_container(db);
                value.hir(&cont, &cont_id.to_container_src_map(db))?.name.clone()
            }
            ModuleDefOrigin::GenerateBlockId(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            ModuleDefOrigin::SubroutineId(subroutine_id) => {
                db.subroutine(subroutine_id).name.clone()
            }
            ModuleDefOrigin::SubroutinePort(InSubroutine { subroutine, value }) => {
                db.subroutine(subroutine).ports.get(value.0 as usize)?.name.clone()
            }
            ModuleDefOrigin::NonAnsiPort(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).label.clone()
            }
            ModuleDefOrigin::Decl(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::Typedef(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::Instance(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            ModuleDefOrigin::Stmt(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).label.clone()
            }
        }
    }

    pub fn name_range(&self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        match *self {
            ModuleDefOrigin::ModuleId(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            ModuleDefOrigin::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            ModuleDefOrigin::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            ModuleDefOrigin::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            ModuleDefOrigin::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            ModuleDefOrigin::GenerateBlockId(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            ModuleDefOrigin::SubroutineId(subroutine_id) => {
                let src = subroutine_id.lookup(db).src;
                Some(InFile::new(src.file_id, src.value.name_or_full_range()))
            }
            ModuleDefOrigin::SubroutinePort(InSubroutine { subroutine, value }) => {
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
            ModuleDefOrigin::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            ModuleDefOrigin::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            ModuleDefOrigin::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            ModuleDefOrigin::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            ModuleDefOrigin::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
        }
    }

    pub fn range(&self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        Some(match *self {
            ModuleDefOrigin::ModuleId(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            ModuleDefOrigin::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            ModuleDefOrigin::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            ModuleDefOrigin::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            ModuleDefOrigin::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            ModuleDefOrigin::GenerateBlockId(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            ModuleDefOrigin::SubroutineId(subroutine_id) => {
                let src = subroutine_id.lookup(db).src;
                let range = src.value.range();
                InFile::new(src.file_id, range)
            }
            ModuleDefOrigin::SubroutinePort(InSubroutine { subroutine, value }) => {
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
            ModuleDefOrigin::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            ModuleDefOrigin::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            ModuleDefOrigin::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            ModuleDefOrigin::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            ModuleDefOrigin::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleDef {
    origins: Box<[ModuleDefOrigin]>,
}

impl ModuleDef {
    pub fn from_origins(origins: impl IntoIterator<Item = ModuleDefOrigin>) -> Option<ModuleDef> {
        let mut origins = origins.into_iter().collect::<SmallVec<[_; 3]>>();
        origins.sort_unstable();
        origins.dedup();

        (!origins.is_empty()).then(|| ModuleDef { origins: origins.into_vec().into_boxed_slice() })
    }

    pub fn origins(&self) -> &[ModuleDefOrigin] {
        &self.origins
    }

    fn into_origins(self) -> Box<[ModuleDefOrigin]> {
        self.origins
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct ModuleDefId(pub salsa::InternId);

impl ModuleDefId {
    pub fn origins(self, db: &dyn HirDb) -> Box<[ModuleDefOrigin]> {
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
    let ModuleDefOrigin::Decl(decl_id) = origin else {
        return false;
    };
    matches!(
        decl_id.cont_id.to_container(db).get(decl_id.value).parent,
        DeclaratorParent::PortDeclId(_)
    )
}
