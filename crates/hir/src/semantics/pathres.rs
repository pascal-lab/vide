use itertools::Itertools;
use la_arena::Idx;
use syntax::ast::{self, support::AstChildren, AstNode};

use crate::{
    container::{ContainerParent, InBlock, InContainer, InFile, InModule},
    hir_def::{
        block::BlockId, data::SubDecl, lower::lower_ident, module::module_item::HierarchicalInst,
        stmt::Stmt, ModuleId,
    },
    scope::{BlockScopeEntry, ModuleScopeEntry, ScopeEntry, ScopeId, UnitScopeEntry},
};

use super::SemanticsImpl;

impl<'db> SemanticsImpl<'db> {
    pub fn resolve_ident(&self, ident: &ast::Identifier) -> Option<PathResolution> {
        self.with_ctx(|ctx| {
            let file_id = self.find_file(ident.syntax());
            let cont_id = ctx.find_container(InFile::new(file_id, *ident.syntax()))?;

            let mut scopes = ContainerParent::new(ctx.db, cont_id)
                .map(|container_id| self.scope_for_container(container_id));
            let ident = lower_ident(ident, ctx.db.hir_file_text(file_id).as_ref())?;

            let res = match scopes.find_map(|scope| Some((scope.id(), scope.get_entry(&ident)?)))? {
                (ScopeId::UnitId(_), ScopeEntry::UnitScopeEntry(entry)) => entry.into(),
                (ScopeId::ModuleId(module_id), ScopeEntry::ModuleScopeEntry(entry)) => {
                    InModule::new(module_id, entry).into()
                }
                (ScopeId::BlockId(block_id), ScopeEntry::BlockScopeEntry(entry)) => {
                    InBlock::new(block_id, entry).into()
                }
                _ => unreachable!(),
            };
            Some(res)
        })
    }

    pub fn resolve_path(&self, path: AstChildren<ast::Identifier>) -> Option<PathResolution> {
        let path = path.collect_vec();
        let last_ident = path.last()?;

        if path.len() == 1 {
            self.resolve_ident(last_ident)
        } else {
            unimplemented!()
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PathResolution {
    ModuleId(ModuleId),
    BlockId(BlockId),
    PortDecl { port: Idx<SubDecl>, data: Option<Idx<SubDecl>>, module_id: ModuleId },
    HierarchyInst(InModule<Idx<HierarchicalInst>>),
    SubDecl(InContainer<Idx<SubDecl>>),
    Stmt(InContainer<Idx<Stmt>>),
}

impl From<UnitScopeEntry> for PathResolution {
    fn from(entry: UnitScopeEntry) -> Self {
        use UnitScopeEntry::*;
        match entry {
            Module(module) => Self::ModuleId(module),
        }
    }
}

impl From<InModule<ModuleScopeEntry>> for PathResolution {
    fn from(entry: InModule<ModuleScopeEntry>) -> Self {
        use ModuleScopeEntry::*;
        let module_id = entry.module_id;
        match entry.value {
            SubDecl(sub_decl) => Self::SubDecl(entry.with_value(sub_decl).into()),
            HierarchyInst(inst) => Self::HierarchyInst(InModule { value: inst, module_id }),
            Block(block_id) => Self::BlockId(block_id),
            Stmt(stmt) => Self::Stmt(entry.with_value(stmt).into()),
            PortDecl { port, data } => Self::PortDecl { port, data, module_id },
        }
    }
}

impl From<InBlock<BlockScopeEntry>> for PathResolution {
    fn from(entry: InBlock<BlockScopeEntry>) -> Self {
        use BlockScopeEntry::*;
        match entry.value {
            SubDecl(sub_decl) => Self::SubDecl(entry.with_value(sub_decl).into()),
            Block(block_id) => Self::BlockId(block_id),
            Stmt(stmt) => Self::Stmt(entry.with_value(stmt).into()),
        }
    }
}
