use smallvec::{SmallVec, smallvec};
use syntax::{SyntaxNode, SyntaxTokenWithParent};

use super::SemanticsImpl;
use crate::{
    container::{
        ContainerId, ContainerParent, InBlock, InContainer, InFile, InGenerateBlock, InModule,
        InSubroutine,
    },
    db::HirDb,
    def_id::{ModuleDef, ModuleDefId, ModuleDefOrigin},
    file::HirFileId,
    hir_def::{
        Ident,
        block::BlockId,
        expr::declarator::DeclId,
        file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
        lower_ident_opt,
        module::{
            ModuleId, generate::GenerateBlockId, instantiation::InstanceId, port::NonAnsiPortId,
        },
        stmt::StmtId,
        subroutine::{SubroutineId, SubroutinePortId},
        typedef::TypedefId,
    },
    scope::{self, BlockEntry, GenerateBlockEntry, ModuleEntry, SubroutineEntry, UnitEntry},
};

impl SemanticsImpl<'_> {
    pub fn nameres_ident(
        &self,
        file_id: HirFileId,
        SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
    ) -> Option<PathResolution> {
        let ident = lower_ident_opt(Some(tok))?;
        self.with_ctx(|ctx| {
            let container = ctx.find_container(InFile::new(file_id, parent));
            ctx.name_to_def(InContainer::new(container, ident))
        })
    }

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> ContainerId {
        self.with_ctx(|ctx| ctx.find_container(node))
    }

    pub fn resolve_name(&self, cont_id: ContainerId, ident: &Ident) -> Option<PathResolution> {
        resolve_name(self.db, cont_id, ident)
    }
}

pub fn resolve_name(db: &dyn HirDb, cont_id: ContainerId, ident: &Ident) -> Option<PathResolution> {
    ContainerParent::start_from(db, cont_id).find_map(|id| match id {
        ContainerId::HirFileId(_) => db.unit_scope().get(ident).map(PathResolution::from),
        ContainerId::ModuleId(module_id) => db
            .module_scope(module_id)
            .get(ident)
            .map(|entry| PathResolution::from(InModule::new(module_id, entry))),
        ContainerId::GenerateBlockId(generate_block_id) => db
            .generate_block_scope(generate_block_id)
            .get(ident)
            .map(|entry| PathResolution::from(InGenerateBlock::new(generate_block_id, entry))),
        ContainerId::BlockId(block_id) => db
            .block_scope(block_id)
            .get(ident)
            .map(|entry| PathResolution::from(InBlock::new(block_id, entry))),
        ContainerId::SubroutineId(subroutine_id) => db
            .subroutine_scope(subroutine_id)
            .get(ident)
            .map(|entry| PathResolution::from(InSubroutine::new(subroutine_id, entry))),
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PathResolution {
    Module(ModuleId),
    Config(InFile<ConfigDeclId>),
    Library(InFile<LibraryDeclId>),
    Udp(InFile<UdpDeclId>),
    Decl(InContainer<DeclId>),
    Typedef(InContainer<TypedefId>),
    ParamDecl(InModule<DeclId>),
    Subroutine(SubroutineId),
    SubroutinePort(InSubroutine<SubroutinePortId>),
    NonAnsiPort {
        // There won't be a situation where all fields are None.
        label: Option<NonAnsiPortId>,
        port_decl: Option<DeclId>,
        data_decl: Option<DeclId>,
        module: ModuleId,
    },
    AnsiPort(InModule<DeclId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
}

impl PathResolution {
    pub fn to_def_id(self, db: &dyn HirDb) -> Option<ModuleDefId> {
        let module_def = ModuleDef::from_origins(self.origins())?;
        Some(db.intern_module_def(module_def))
    }

    fn origins(self) -> SmallVec<[ModuleDefOrigin; 3]> {
        let mut res = smallvec![];
        let mut add_source = |source| res.push(source);

        match self {
            PathResolution::NonAnsiPort { label, port_decl, data_decl, module } => {
                let container: ContainerId = module.into();
                if let Some(label) = label {
                    add_source(InModule::new(module, label).into());
                }
                if let Some(port_decl) = port_decl {
                    add_source(InContainer::new(container, port_decl).into());
                }
                if let Some(decl) = data_decl {
                    add_source(InContainer::new(container, decl).into());
                }
            }
            _ => {
                if let Some(origin) = self.pick() {
                    add_source(origin);
                }
            }
        };

        res
    }

    #[inline]
    fn pick(self) -> Option<ModuleDefOrigin> {
        match self {
            PathResolution::Module(module_id) => Some(module_id.into()),
            PathResolution::Config(config_id) => Some(config_id.into()),
            PathResolution::Library(library_id) => Some(library_id.into()),
            PathResolution::Udp(udp_id) => Some(udp_id.into()),
            PathResolution::Decl(decl_id) => Some(decl_id.into()),
            PathResolution::Typedef(typedef_id) => Some(typedef_id.into()),
            PathResolution::Instance(instance_id) => Some(instance_id.into()),
            PathResolution::Stmt(stmt_id) => Some(stmt_id.into()),
            PathResolution::Block(blk_id) => Some(blk_id.into()),
            PathResolution::GenerateBlock(generate_block_id) => Some(generate_block_id.into()),
            PathResolution::Subroutine(subroutine_id) => Some(subroutine_id.into()),
            PathResolution::SubroutinePort(port_id) => Some(port_id.into()),
            PathResolution::ParamDecl(decl_id) | PathResolution::AnsiPort(decl_id) => {
                Some(InContainer::new(decl_id.module_id.into(), decl_id.value).into())
            }
            PathResolution::NonAnsiPort { label, port_decl, data_decl, module } => {
                let container: ContainerId = module.into();
                if let Some(label) = label {
                    Some(InModule::new(module, label).into())
                } else if let Some(port_decl) = port_decl {
                    Some(InContainer::new(container, port_decl).into())
                } else {
                    data_decl.map(|decl| InContainer::new(container, decl).into())
                }
            }
        }
    }
}

impl From<UnitEntry> for PathResolution {
    fn from(entry: UnitEntry) -> Self {
        use UnitEntry::*;
        match entry {
            ModuleId(idx) => Self::Module(idx),
            FiledConfigDeclId(idx) => Self::Config(idx),
            FiledLibraryDeclId(idx) => Self::Library(idx),
            FiledUdpDeclId(idx) => Self::Udp(idx),
            FiledDeclId(idx) => Self::Decl(idx.into()),
            FiledTypedefId(idx) => Self::Typedef(idx.into()),
        }
    }
}

impl From<InModule<ModuleEntry>> for PathResolution {
    fn from(entry: InModule<ModuleEntry>) -> Self {
        use ModuleEntry::*;
        match entry.value {
            DeclId(decl_id) => Self::Decl(entry.with_value(decl_id).into()),
            TypedefId(typedef_id) => Self::Typedef(entry.with_value(typedef_id).into()),
            InstanceId(idx) => Self::Instance(entry.with_value(idx)),
            GenerateBlockId(generate_block_id) => Self::GenerateBlock(generate_block_id),
            StmtId(idx) => Self::Stmt(entry.with_value(idx).into()),
            SubroutineId(subroutine_id) => Self::Subroutine(subroutine_id),
            NonAnsiPortEntry(scope::NonAnsiPortEntry { label, port_decl, data_decl }) => {
                Self::NonAnsiPort { label, port_decl, data_decl, module: entry.module_id }
            }
            AnsiPortEntry(scope::AnsiPortEntry(idx)) => Self::AnsiPort(entry.with_value(idx)),
            BlockId(block_id) => Self::Block(block_id),
        }
    }
}

impl From<InGenerateBlock<GenerateBlockEntry>> for PathResolution {
    fn from(entry: InGenerateBlock<GenerateBlockEntry>) -> Self {
        use GenerateBlockEntry::*;
        match entry.value {
            DeclId(idx) => Self::Decl(entry.with_value(idx).into()),
            TypedefId(idx) => Self::Typedef(entry.with_value(idx).into()),
            GenerateBlockId(generate_block_id) => Self::GenerateBlock(generate_block_id),
            StmtId(idx) => Self::Stmt(entry.with_value(idx).into()),
            BlockId(block_id) => Self::Block(block_id),
            SubroutineId(subroutine_id) => Self::Subroutine(subroutine_id),
        }
    }
}

impl From<InBlock<BlockEntry>> for PathResolution {
    fn from(entry: InBlock<BlockEntry>) -> Self {
        use BlockEntry::*;
        match entry.value {
            DeclId(idx) => Self::Decl(entry.with_value(idx).into()),
            TypedefId(idx) => Self::Typedef(entry.with_value(idx).into()),
            StmtId(idx) => Self::Stmt(entry.with_value(idx).into()),
            BlockId(block_id) => Self::Block(block_id),
        }
    }
}

impl From<InSubroutine<SubroutineEntry>> for PathResolution {
    fn from(entry: InSubroutine<SubroutineEntry>) -> Self {
        use SubroutineEntry::*;
        match entry.value {
            DeclId(idx) => Self::Decl(entry.with_value(idx).into()),
            TypedefId(idx) => Self::Typedef(entry.with_value(idx).into()),
            StmtId(idx) => Self::Stmt(entry.with_value(idx).into()),
            BlockId(block_id) => Self::Block(block_id),
            SubroutinePortId(idx) => Self::SubroutinePort(entry.with_value(idx)),
        }
    }
}
