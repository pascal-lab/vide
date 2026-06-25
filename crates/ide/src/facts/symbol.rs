use hir::{
    base_db::intern::Lookup,
    container::{ContainerId, InContainer, InFile, InModule, InSubroutine},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        block::{BlockId, BlockLoc},
        declaration::Declaration,
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
    preproc::MacroDefinitionId,
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
use vfs::FileId;

use crate::{FileRange, SymbolKind};

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
    PreprocMacro {
        id: MacroDefinitionId,
        file_id: FileId,
        name_range: TextRange,
        directive_range: TextRange,
    },
    Include {
        source_file: FileId,
        included_file: Option<FileId>,
        range: TextRange,
    },
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
    pub fn info(&self, db: &dyn HirDb) -> Option<SymbolInfo> {
        Some(SymbolInfo {
            id: *self,
            name: self.name(db),
            kind: self.kind(db),
            definition_range: self.range(db).map(into_file_range),
            selection_range: self.name_range(db).map(into_file_range),
            container: self.container_symbol(db),
        })
    }

    pub fn kind(&self, db: &dyn HirDb) -> SymbolKind {
        match *self {
            SymbolId::ModuleId(_) => SymbolKind::Module,
            SymbolId::Config(_) => SymbolKind::Config,
            SymbolId::Library(_) => SymbolKind::Library,
            SymbolId::Udp(_) => SymbolKind::Primitive,
            SymbolId::BlockId(_) => SymbolKind::Block,
            SymbolId::GenerateBlockId(_) => SymbolKind::Generate,
            SymbolId::SubroutineId(_) => SymbolKind::Fn,
            SymbolId::SubroutinePort(_) => SymbolKind::PortDecl,
            SymbolId::NonAnsiPort(_) => SymbolKind::NonAnsiPortLabel,
            SymbolId::Decl(InContainer { value, cont_id }) => {
                let cont = cont_id.to_container(db);
                let decl = cont.get(value);
                match decl.parent {
                    DeclaratorParent::PortDeclId(_) => SymbolKind::PortDecl,
                    DeclaratorParent::DeclarationId(idx) => match cont.get(idx) {
                        Declaration::DataDecl(_) => SymbolKind::DataDecl,
                        Declaration::NetDecl(_) => SymbolKind::NetDecl,
                        Declaration::ParamDecl(_) => SymbolKind::ParamDecl,
                        Declaration::GenvarDecl(_) => SymbolKind::Genvar,
                        Declaration::SpecparamDecl(_) => SymbolKind::Specparam,
                    },
                    DeclaratorParent::StmtId(_) => SymbolKind::DataDecl,
                }
            }
            SymbolId::Typedef(_) => SymbolKind::Typedef,
            SymbolId::Instance(_) => SymbolKind::Instance,
            SymbolId::Stmt(_) => SymbolKind::Stmt,
            SymbolId::PreprocMacro { .. } => SymbolKind::Macro,
            SymbolId::Include { .. } => SymbolKind::Include,
        }
    }

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
            SymbolId::PreprocMacro { file_id, .. }
            | SymbolId::Include { source_file: file_id, .. } => HirFileId::File(file_id).into(),
        }
    }

    fn container_symbol(&self, db: &dyn HirDb) -> Option<SymbolId> {
        container_symbol_id(self.container_id(db), db)
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
            SymbolId::PreprocMacro { file_id, name_range, .. } => {
                text_for_range(db, file_id, name_range)
            }
            SymbolId::Include { source_file, range, .. } => text_for_range(db, source_file, range),
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
            SymbolId::PreprocMacro { file_id, name_range, .. } => {
                Some(InFile::new(HirFileId::File(file_id), name_range))
            }
            SymbolId::Include { source_file, range, .. } => {
                Some(InFile::new(HirFileId::File(source_file), range))
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
            SymbolId::PreprocMacro { file_id, directive_range, .. } => {
                InFile::new(HirFileId::File(file_id), directive_range)
            }
            SymbolId::Include { source_file, range, .. } => {
                InFile::new(HirFileId::File(source_file), range)
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInfo {
    pub id: SymbolId,
    pub name: Option<SmolStr>,
    pub kind: SymbolKind,
    pub definition_range: Option<FileRange>,
    pub selection_range: Option<FileRange>,
    pub container: Option<SymbolId>,
}

fn container_symbol_id(container_id: ContainerId, db: &dyn HirDb) -> Option<SymbolId> {
    match container_id {
        ContainerId::HirFileId(_) => None,
        ContainerId::ModuleId(module_id) => Some(SymbolId::ModuleId(module_id)),
        ContainerId::GenerateBlockId(generate_block_id) => {
            Some(SymbolId::GenerateBlockId(generate_block_id))
        }
        ContainerId::BlockId(block_id) => Some(SymbolId::BlockId(block_id)),
        ContainerId::SubroutineId(subroutine_id) => {
            let _ = db;
            Some(SymbolId::SubroutineId(subroutine_id))
        }
    }
}

fn into_file_range(InFile { file_id, value: range }: InFile<TextRange>) -> FileRange {
    FileRange { file_id: file_id.file_id(), range }
}

fn text_for_range(db: &dyn HirDb, file_id: FileId, range: TextRange) -> Option<SmolStr> {
    let text = db.file_text(file_id);
    Some(SmolStr::new(&text[range]))
}
