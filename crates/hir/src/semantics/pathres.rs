use smallvec::SmallVec;
use syntax::{SyntaxNode, SyntaxTokenWithParent};
use triomphe::Arc;

use super::SemanticsImpl;
use crate::{
    container::{InContainer, InFile, ScopeId, ScopeParent},
    db::HirDb,
    def_id::{ModuleDef, ModuleDefId},
    file::HirFileId,
    hir_def::{Ident, lower_ident_opt},
    symbol::{DefId, NameScope},
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

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> ScopeId {
        self.with_ctx(|ctx| ctx.find_container(node))
    }

    pub fn resolve_name(&self, cont_id: ScopeId, ident: &Ident) -> Option<PathResolution> {
        resolve_name(self.db, cont_id, ident)
    }
}

pub fn resolve_name(db: &dyn HirDb, cont_id: ScopeId, ident: &Ident) -> Option<PathResolution> {
    ScopeParent::start_from(db, cont_id).find_map(|id| {
        let scope = name_scope(db, id);
        scope.lookup_merged(ident).and_then(PathResolution::from_def_ids)
    })
}

pub(crate) fn name_scope(db: &dyn HirDb, scope_id: ScopeId) -> Arc<NameScope> {
    match scope_id {
        ScopeId::File(_) => db.unit_scope(),
        ScopeId::Module(module_id) => db.module_scope(module_id),
        ScopeId::GenerateBlock(generate_block_id) => db.generate_block_scope(generate_block_id),
        ScopeId::Block(block_id) => db.block_scope(block_id),
        ScopeId::Subroutine(subroutine_id) => db.subroutine_scope(subroutine_id),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathResolution {
    def_ids: SmallVec<[DefId; 3]>,
}

impl PathResolution {
    pub fn from_def_id(def_id: DefId) -> Self {
        Self { def_ids: SmallVec::from_slice(&[def_id]) }
    }

    pub fn from_def_ids(def_ids: impl IntoIterator<Item = DefId>) -> Option<Self> {
        let mut resolved = SmallVec::<[DefId; 3]>::new();
        for def_id in def_ids {
            if !resolved.contains(&def_id) {
                resolved.push(def_id);
            }
        }
        (!resolved.is_empty()).then_some(Self { def_ids: resolved })
    }

    pub fn def_ids(&self) -> &[DefId] {
        &self.def_ids
    }

    pub fn primary_def_id(&self) -> Option<DefId> {
        self.def_ids.first().copied()
    }

    pub fn to_def_id(&self, db: &dyn HirDb) -> Option<ModuleDefId> {
        let module_def = ModuleDef::from_def_ids(self.def_ids.iter().copied())?;
        Some(db.intern_module_def(module_def))
    }
}

impl From<DefId> for PathResolution {
    fn from(def_id: DefId) -> Self {
        Self::from_def_id(def_id)
    }
}
