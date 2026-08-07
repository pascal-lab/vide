use hir_def::{
    Ident,
    container::{InContainer, InFile},
    def_id::DefId,
    lower_ident_opt,
    owner::OwnerId,
    pathres::{ResolvedScopes, resolve_in_resolved_scopes},
    symbol::{NameContext, Resolution},
};
use preproc_expand::file::HirFileId;
use syntax::{SyntaxNode, SyntaxTokenWithParent};

use super::{SemanticsImpl, hir_to_def, source_to_def};

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
        let container = source_to_def::find_container(self.db, InFile::new(file_id, parent));
        hir_to_def::name_to_def(self.db, InContainer::new(container, ident), name_ctx)
    }

    /// Like [`nameres_ident`](Self::nameres_ident), but resolves inside a
    /// caller-provided container instead of re-walking the ancestor chain to
    /// find it. Callers that process many tokens of one tree (the semantic
    /// index build) track the container while walking the tree and pass it
    /// here; the result is identical to `nameres_ident`.
    pub fn nameres_ident_in(
        &self,
        _file_id: HirFileId,
        SyntaxTokenWithParent { tok, .. }: SyntaxTokenWithParent,
        name_ctx: NameContext,
        container: OwnerId,
    ) -> Resolution<DefId> {
        let Some(ident) = lower_ident_opt(Some(tok)) else {
            return Resolution::Unresolved;
        };
        hir_to_def::name_to_def(self.db, InContainer::new(container, ident), name_ctx)
    }

    /// Like [`nameres_ident_in`](Self::nameres_ident_in), but looks up the
    /// name in an already-resolved scope chain, avoiding the per-token salsa
    /// `scope_for` queries (whose memos revalidate against every intervening
    /// query during the index build). The result is identical to
    /// `nameres_ident_in` as long as the chain matches the container.
    pub fn nameres_ident_in_scopes(
        &self,
        SyntaxTokenWithParent { tok, .. }: SyntaxTokenWithParent,
        name_ctx: NameContext,
        resolved: &ResolvedScopes,
    ) -> Resolution<DefId> {
        let Some(ident) = lower_ident_opt(Some(tok)) else {
            return Resolution::Unresolved;
        };
        resolve_in_resolved_scopes(self.db, resolved, &ident, name_ctx)
    }

    pub(in crate::semantics) fn find_container(&self, node: InFile<SyntaxNode>) -> OwnerId {
        source_to_def::find_container(self.db, node)
    }

    pub fn resolve_name(
        &self,
        owner: OwnerId,
        ident: &Ident,
        ctx: NameContext,
    ) -> Resolution<DefId> {
        hir_def::pathres::resolve_name(self.db, owner, ident, ctx)
    }
}
