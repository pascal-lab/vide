use hir::{
    base_db::intern::Lookup,
    container::{ContainerId, InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    hir_def::{
        block::{BlockId, BlockLoc},
        expr::declarator::DeclId,
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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum SymbolId {
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

pub type DefinitionOrigin = SymbolId;

impl_from! { SymbolId =>
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

impl SymbolId {
    #[inline]
    pub fn container_id(&self, db: &dyn HirDb) -> ContainerId {
        match *self {
            SymbolId::ModuleId(InFile { file_id, .. }) => file_id.into(),
            SymbolId::Config(InFile { file_id, .. }) => file_id.into(),
            SymbolId::Library(InFile { file_id, .. }) => file_id.into(),
            SymbolId::Udp(InFile { file_id, .. }) => file_id.into(),
            SymbolId::BlockId(block_id) => block_id.lookup(db).cont_id,
            SymbolId::GenerateBlockId(generate_block_id) => generate_block_id.lookup(db).cont_id,
            SymbolId::SubroutineId(subroutine_id) => subroutine_id.lookup(db).cont_id.into(),
            SymbolId::SubroutinePort(InSubroutine { subroutine, .. }) => {
                ContainerId::SubroutineId(subroutine)
            }
            SymbolId::NonAnsiPort(InModule { module_id, .. }) => module_id.into(),
            SymbolId::Decl(InContainer { cont_id, .. }) => cont_id,
            SymbolId::Typedef(InContainer { cont_id, .. }) => cont_id,
            SymbolId::Instance(InModule { module_id, .. }) => module_id.into(),
            SymbolId::Stmt(InContainer { cont_id, .. }) => cont_id,
        }
    }

    pub fn name(&self, db: &dyn HirDb) -> Option<SmolStr> {
        match *self {
            SymbolId::ModuleId(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            SymbolId::Config(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            SymbolId::Library(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            SymbolId::Udp(InFile { value, file_id }) => {
                file_id.to_container(db).get(value).name.clone()
            }
            SymbolId::BlockId(block_id) => {
                let BlockLoc { cont_id, src: InFile { value, file_id: _ } } = block_id.lookup(db);
                let cont = cont_id.to_container(db);
                value.hir(&cont, &cont_id.to_container_src_map(db))?.name.clone()
            }
            SymbolId::GenerateBlockId(generate_block_id) => {
                db.generate_block(generate_block_id).name.clone()
            }
            SymbolId::SubroutineId(subroutine_id) => db.subroutine(subroutine_id).name.clone(),
            SymbolId::SubroutinePort(InSubroutine { subroutine, value }) => {
                db.subroutine(subroutine).ports.get(value.0 as usize)?.name.clone()
            }
            SymbolId::NonAnsiPort(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).label.clone()
            }
            SymbolId::Decl(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
            }
            SymbolId::Typedef(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).name.clone()
            }
            SymbolId::Instance(InModule { value, module_id }) => {
                module_id.to_container(db).get(value).name.clone()
            }
            SymbolId::Stmt(InContainer { value, cont_id }) => {
                cont_id.to_container(db).get(value).label.clone()
            }
        }
    }

    pub fn name_range(&self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        match *self {
            SymbolId::ModuleId(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            SymbolId::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            SymbolId::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            SymbolId::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(file_id, range))
            }
            SymbolId::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            SymbolId::GenerateBlockId(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
                let range = value.name_range()?;
                Some(InFile::new(file_id, range))
            }
            SymbolId::SubroutineId(subroutine_id) => {
                let src = subroutine_id.lookup(db).src;
                Some(InFile::new(src.file_id, src.value.name_or_full_range()))
            }
            SymbolId::SubroutinePort(InSubroutine { subroutine, value }) => {
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
            SymbolId::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            SymbolId::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            SymbolId::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
            SymbolId::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(module_id.file_id, range))
            }
            SymbolId::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.name_range()?;
                Some(InFile::new(cont_id.file_id(db).into(), range))
            }
        }
    }

    pub fn range(&self, db: &dyn HirDb) -> Option<InFile<TextRange>> {
        Some(match *self {
            SymbolId::ModuleId(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            SymbolId::Config(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            SymbolId::Library(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            SymbolId::Udp(InFile { value, file_id }) => {
                let range = file_id.to_container_src_map(db).get(value)?.range();
                InFile::new(file_id, range)
            }
            SymbolId::BlockId(block_id) => {
                let BlockLoc { src: InFile { value, file_id }, .. } = block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            SymbolId::GenerateBlockId(generate_block_id) => {
                let GenerateBlockLoc { src: InFile { value, file_id }, .. } =
                    generate_block_id.lookup(db);
                let range = value.range();
                InFile::new(file_id, range)
            }
            SymbolId::SubroutineId(subroutine_id) => {
                let src = subroutine_id.lookup(db).src;
                let range = src.value.range();
                InFile::new(src.file_id, range)
            }
            SymbolId::SubroutinePort(InSubroutine { subroutine, value }) => {
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
            SymbolId::NonAnsiPort(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            SymbolId::Decl(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            SymbolId::Typedef(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
            SymbolId::Instance(InModule { value, module_id }) => {
                let range = module_id.to_container_src_map(db).get(value)?.range();
                InFile::new(module_id.file_id, range)
            }
            SymbolId::Stmt(InContainer { value, cont_id }) => {
                let range = cont_id.to_container_src_map(db).get(value)?.range();
                InFile::new(cont_id.file_id(db).into(), range)
            }
        })
    }
}
