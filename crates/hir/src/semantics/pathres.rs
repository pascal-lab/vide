use hir_def::{
    container::{ArenaOwnerId, InContainer, InFile, ScopeId},
    def_id::DefId,
    hir_def::{Ident, lower_ident_opt},
    symbol::{NameContext, Resolution},
};
use preproc_expand::file::HirFileId;
use syntax::{SyntaxNode, SyntaxTokenWithParent};

use super::SemanticsImpl;

impl SemanticsImpl<'_> {
    pub fn nameres_ident(
        &self,
        file_id: HirFileId,
        SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
        name_ctx: NameContext,
    ) -> Resolution<DefId> {
        let Some(ident) = lower_ident_opt(Some(tok)) else {
            return Resolution::Unresolved;
        };
        self.with_ctx(|source_ctx| {
            let container = source_ctx.find_container(InFile::new(file_id, parent));
            source_ctx.name_to_def(InContainer::new(container, ident), name_ctx)
        })
    }

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> ArenaOwnerId {
        self.with_ctx(|ctx| ctx.find_container(node))
    }

    pub fn resolve_name(
        &self,
        cont_id: ScopeId,
        ident: &Ident,
        ctx: NameContext,
    ) -> Resolution<DefId> {
        hir_def::pathres::resolve_name(self.db, cont_id, ident, ctx)
    }
}
