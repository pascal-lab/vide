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
    symbol::{DefId, NameContext, NameScope},
};

impl SemanticsImpl<'_> {
    pub fn nameres_ident(
        &self,
        file_id: HirFileId,
        SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
        name_ctx: NameContext,
    ) -> Option<PathResolution> {
        let ident = lower_ident_opt(Some(tok))?;
        self.with_ctx(|source_ctx| {
            let container = source_ctx.find_container(InFile::new(file_id, parent));
            source_ctx.name_to_def(InContainer::new(container, ident), name_ctx)
        })
    }

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> ScopeId {
        self.with_ctx(|ctx| ctx.find_container(node))
    }

    pub fn resolve_name(
        &self,
        cont_id: ScopeId,
        ident: &Ident,
        ctx: NameContext,
    ) -> Option<PathResolution> {
        resolve_name(self.db, cont_id, ident, ctx)
    }
}

pub fn resolve_name(
    db: &dyn HirDb,
    cont_id: ScopeId,
    ident: &Ident,
    ctx: NameContext,
) -> Option<PathResolution> {
    let scopes = ScopeParent::start_from(db, cont_id).collect::<SmallVec<[_; 4]>>();

    for id in &scopes {
        let scope = name_scope(db, *id);
        if let Some(res) = scope.lookup(ctx, ident).and_then(PathResolution::from_def_ids) {
            return Some(res);
        }
    }

    // IEEE 1800-2017 keeps package imports distinct from ordinary lexical
    // declarations: visible declarations in the lexical chain win, then
    // package imports are considered, and `$unit` remains an explicit outer
    // scope. `NameContext` chooses the namespace bucket at every phase.
    if let Some(res) = resolve_imported_name(db, &scopes, ident, ctx) {
        return Some(res);
    }

    db.unit_scope().lookup(ctx, ident).and_then(PathResolution::from_def_ids)
}

pub(crate) fn name_scope(db: &dyn HirDb, scope_id: ScopeId) -> Arc<NameScope> {
    match scope_id {
        ScopeId::File(file_id) => db.file_scope(file_id),
        ScopeId::Module(module_id) => db.module_scope(module_id),
        ScopeId::GenerateBlock(generate_block_id) => db.generate_block_scope(generate_block_id),
        ScopeId::Block(block_id) => db.block_scope(block_id),
        ScopeId::Subroutine(subroutine_id) => db.subroutine_scope(subroutine_id.as_in_container()),
    }
}

fn resolve_imported_name(
    db: &dyn HirDb,
    scopes: &[ScopeId],
    ident: &Ident,
    ctx: NameContext,
) -> Option<PathResolution> {
    let mut defs = SmallVec::<[DefId; 3]>::new();

    for scope_id in scopes {
        let scope = name_scope(db, *scope_id);
        collect_imports(db, &scope, ident, ctx, true, &mut defs);
        if !defs.is_empty() {
            return PathResolution::from_def_ids(defs);
        }
    }

    for scope_id in scopes {
        let scope = name_scope(db, *scope_id);
        collect_imports(db, &scope, ident, ctx, false, &mut defs);
        if !defs.is_empty() {
            return PathResolution::from_def_ids(defs);
        }
    }

    None
}

fn collect_imports(
    db: &dyn HirDb,
    scope: &NameScope,
    ident: &Ident,
    ctx: NameContext,
    named_only: bool,
    defs: &mut SmallVec<[DefId; 3]>,
) {
    for import in &scope.imports {
        match (&import.name, named_only) {
            (Some(name), true) if name == ident => {}
            (None, false) => {}
            _ => continue,
        }

        let Some(package_id) = db.unit_scope().package_ids(db, &import.package).unique() else {
            continue;
        };
        let package_scope = db.package_export_scope(package_id);
        if let Some(imported) = package_scope.lookup(ctx, ident) {
            for def_id in imported {
                if !defs.contains(&def_id) {
                    defs.push(def_id);
                }
            }
        }
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
