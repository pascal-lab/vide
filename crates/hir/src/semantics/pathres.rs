use syntax::{SyntaxNode, SyntaxTokenWithParent};

use super::SemanticsImpl;
use crate::{
    container::{
        ContainerId, InBlock, InContainer, InFile, InGenerateBlock, InModule, InSubroutine,
    },
    file::HirFileId,
    hir_def::{
        block::BlockId,
        expr::declarator::DeclId,
        file::{config::ConfigDeclId, library::LibraryDeclId, udp::UdpDeclId},
        lower_ident_opt,
        module::{
            ModuleId, generate::GenerateBlockId, instantiation::InstanceId, modport::ModportId,
            port::NonAnsiPortId,
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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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
    Modport(InModule<ModportId>),
    Stmt(InContainer<StmtId>),
    Block(BlockId),
    GenerateBlock(GenerateBlockId),
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
            ModportId(idx) => Self::Modport(entry.with_value(idx)),
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
