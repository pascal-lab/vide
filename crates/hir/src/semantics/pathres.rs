use syntax::{
    SyntaxTokenWithParent,
    ast::{self, AstNode},
};

use super::SemanticsImpl;
use crate::{
    container::{ContainerId, ContainerParent, InBlock, InContainer, InFile, InModule},
    hir_def::{
        block::BlockId,
        expr::declarator::DeclId,
        lower_ident_opt,
        module::{ModuleId, instantiation::InstanceId, port::NonAnsiPortId},
        stmt::StmtId,
    },
    scope::{self, BlockEntry, ModuleEntry, UnitEntry},
};

impl SemanticsImpl<'_> {
    pub fn resolve_ident_in_cont(
        &self,
        SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
    ) -> Option<PathResolution> {
        let db = self.db;
        let file_id = self.find_file(parent);
        let ident = lower_ident_opt(Some(tok))?;
        self.with_ctx(|ctx| {
            let container = ctx.find_container(InFile::new(file_id, parent))?;

            ContainerParent::start_from(db, container).find_map(|id| match id {
                ContainerId::HirFileId(_) => {
                    let scope = db.unit_scope();
                    let entry = scope.get(&ident)?;
                    Some(entry.into())
                }
                ContainerId::ModuleId(module_id) => {
                    let scope = db.module_scope(module_id);
                    let entry = scope.get(&ident)?;
                    Some(InModule::new(module_id, entry).into())
                }
                ContainerId::BlockId(block_id) => {
                    let scope = db.block_scope(block_id);
                    let entry = scope.get(&ident)?;
                    Some(InBlock::new(block_id, entry).into())
                }
            })
        })
    }

    pub fn resolve_port_conn_name(&self, conn: ast::NamedPortConnection) -> Option<PathResolution> {
        let db = self.db;
        let conn_name = lower_ident_opt(conn.name())?;

        let instantiatiion = ast::HierarchyInstantiation::cast(conn.syntax().parent()?.parent()?)?;
        let module_name = lower_ident_opt(instantiatiion.type_())?;
        let UnitEntry::ModuleId(module_id) = db.unit_scope().get(&module_name)? else {
            return None;
        };

        let module_scope = db.module_scope(module_id);
        let entry = module_scope.get(&conn_name)?;

        if matches!(entry, ModuleEntry::AnsiPortEntry(_) | ModuleEntry::NonAnsiPortEntry(_)) {
            Some(InModule::new(module_id, entry).into())
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PathResolution {
    Module(ModuleId),
    Decl(InContainer<DeclId>),
    NonAnsiPort {
        label: Option<NonAnsiPortId>,
        port_decl: Option<DeclId>,
        data_decl: Option<DeclId>,
        module: ModuleId,
    },
    AnsiPort(InModule<DeclId>),
    Instance(InModule<InstanceId>),
    Stmt(InContainer<StmtId>),
    Block(BlockId),
}

impl From<UnitEntry> for PathResolution {
    fn from(entry: UnitEntry) -> Self {
        use UnitEntry::*;
        match entry {
            ModuleId(idx) => Self::Module(idx),
            FiledDeclId(idx) => Self::Decl(idx.into()),
        }
    }
}

impl From<InModule<ModuleEntry>> for PathResolution {
    fn from(entry: InModule<ModuleEntry>) -> Self {
        use ModuleEntry::*;
        match entry.value {
            DeclId(decl_id) => Self::Decl(entry.with_value(decl_id).into()),
            InstanceId(idx) => Self::Instance(entry.with_value(idx)),
            StmtId(idx) => Self::Stmt(entry.with_value(idx).into()),
            NonAnsiPortEntry(scope::NonAnsiPortEntry { label, port_decl, data_decl }) => {
                Self::NonAnsiPort { label, port_decl, data_decl, module: entry.cont_id }
            }
            AnsiPortEntry(scope::AnsiPortEntry(idx)) => Self::AnsiPort(entry.with_value(idx)),
            BlockId(block_id) => Self::Block(block_id),
        }
    }
}

impl From<InBlock<BlockEntry>> for PathResolution {
    fn from(entry: InBlock<BlockEntry>) -> Self {
        use BlockEntry::*;
        match entry.value {
            DeclId(idx) => Self::Decl(entry.with_value(idx).into()),
            StmtId(idx) => Self::Stmt(entry.with_value(idx).into()),
            BlockId(block_id) => Self::Block(block_id),
        }
    }
}
