use hir_def::{
    Ident,
    container::{InFile, OwnerRef},
    db::HirDefDb,
    def_id::DefId,
    lower_ident_opt,
    owner::OwnerId,
    pathres::{NameRef, RefKind, ResolvedScopes, resolve_in_resolved_scopes_at},
    symbol::{NameContext, Resolution},
};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxTokenWithParent,
    ast::{self, AstNode},
};

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
        let reference = token_reference(self.db, file_id, parent);
        hir_to_def::name_to_def_at(
            self.db,
            OwnerRef::new(container, ident),
            name_ctx,
            reference.as_ref(),
        )
    }

    /// Like [`nameres_ident`](Self::nameres_ident), but resolves inside a
    /// caller-provided container instead of re-walking the ancestor chain to
    /// find it. Callers that process many tokens of one tree (the semantic
    /// index build) track the container while walking the tree and pass it
    /// here; the result is identical to `nameres_ident`.
    pub fn nameres_ident_in(
        &self,
        file_id: HirFileId,
        SyntaxTokenWithParent { parent, tok }: SyntaxTokenWithParent,
        name_ctx: NameContext,
        container: OwnerId,
    ) -> Resolution<DefId> {
        let Some(ident) = lower_ident_opt(Some(tok)) else {
            return Resolution::Unresolved;
        };
        let reference = token_reference(self.db, file_id, parent);
        hir_to_def::name_to_def_at(
            self.db,
            OwnerRef::new(container, ident),
            name_ctx,
            reference.as_ref(),
        )
    }

    /// Like [`nameres_ident_in`](Self::nameres_ident_in), but looks up the
    /// name in an already-resolved scope chain, avoiding the per-token salsa
    /// `scope_for` queries (whose memos revalidate against every intervening
    /// query during the index build). The result is identical to
    /// `nameres_ident_in` as long as the chain matches the container.
    pub fn nameres_ident_in_scopes(
        &self,
        SyntaxTokenWithParent { parent: _, tok }: SyntaxTokenWithParent,
        name_ctx: NameContext,
        resolved: &ResolvedScopes,
        reference: Option<&NameRef>,
    ) -> Resolution<DefId> {
        let Some(ident) = lower_ident_opt(Some(tok)) else {
            return Resolution::Unresolved;
        };
        resolve_in_resolved_scopes_at(self.db, resolved, &ident, name_ctx, reference)
    }

    /// Token-level variant of [`nameres_ident_in_scopes`] that derives the
    /// reference position from the token itself; declaration names resolve
    /// position-less (see [`token_reference`]).
    pub fn nameres_ident_in_scopes_at(
        &self,
        file_id: HirFileId,
        token: SyntaxTokenWithParent,
        name_ctx: NameContext,
        resolved: &ResolvedScopes,
    ) -> Resolution<DefId> {
        let SyntaxTokenWithParent { parent, tok } = token;
        let reference = token_reference(self.db, file_id, parent);
        self.nameres_ident_in_scopes(
            SyntaxTokenWithParent { parent, tok },
            name_ctx,
            resolved,
            reference.as_ref(),
        )
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

    /// Position-aware name resolution honoring the reference point (IEEE
    /// 1800-2017 26.3); see [`hir_def::pathres::resolve_name_at`].
    pub fn resolve_name_at(
        &self,
        owner: OwnerId,
        ident: &Ident,
        ctx: NameContext,
        reference: Option<&hir_def::pathres::NameRef>,
    ) -> Resolution<DefId> {
        hir_def::pathres::resolve_name_at(self.db, owner, ident, ctx, reference)
    }
}

/// Reference position and call-ness of a name token, for point-of-reference
/// resolution (IEEE 1800-2017 26.3). Structured: a token whose parent is an
/// expression is a reference (declaration names sit inside declarators), and
/// a reference is a call when it is exactly the callee expression of the
/// nearest invocation.
pub fn token_reference(
    db: &dyn HirDefDb,
    file_id: HirFileId,
    parent: SyntaxNode<'_>,
) -> Option<NameRef> {
    let expression = ast::Expression::cast(parent)?;
    let node = expression.syntax();
    let position = InFile::new(file_id, db.ast_id_map(file_id).id_of_node(node)?);
    let kind = SyntaxAncestors::start_from(parent)
        .find_map(ast::InvocationExpression::cast)
        .map(
            |invocation| {
                if invocation.left().syntax() == node { RefKind::Call } else { RefKind::Value }
            },
        )
        .unwrap_or(RefKind::Value);
    Some(NameRef { position, kind })
}
