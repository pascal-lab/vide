use hir_def::symbol::NameContext;
use hir_semantics::semantics::Semantics;
use hir_ty::{Member, TypeSystem};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use super::candidate::CompletionCandidate;
use crate::{
    FilePosition, analysis::AnalysisContext, completion::context::CompletionContext,
    db::root_db::RootDb,
};

pub(super) fn complete_member_access(
    db: &AnalysisContext<'_>,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let sema = db.semantics();
    let file_id = position.file_id.into();
    let parsed_file = sema.parse_file(position.file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };

    let members = member_access_at_offset(root, position.offset)
        .and_then(|access| members_for_expr(db, &sema, file_id, access.left()))
        .or_else(|| members_for_incomplete_access(db, &sema, file_id, root, position.offset))
        .or_else(|| members_for_incomplete_scoped_access(db, &sema, file_id, root, position.offset))
        .or_else(|| {
            scoped_name_at_offset(root, position.offset)
                .and_then(|scoped| members_for_scoped_name(db, &sema, file_id, scoped))
        });
    let Some(members) = members else {
        return Vec::new();
    };

    members
        .into_iter()
        .map(Member::into_name)
        .filter(|name| name.as_str().starts_with(prefix))
        .map(|name| {
            let label = name.to_string();
            CompletionCandidate::text(label, ctx.replacement)
        })
        .collect()
}

fn member_access_at_offset(
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<ast::MemberAccessExpression<'_>> {
    let prev = root.token_before_offset(offset)?;
    if prev.kind() != syntax::Token![.] {
        return None;
    }
    SyntaxAncestors::start_from(prev.parent).find_map(ast::MemberAccessExpression::cast)
}

fn scoped_name_at_offset(
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<ast::ScopedName<'_>> {
    let elem = root.covering_element(utils::line_index::TextRange::empty(offset));
    let node = elem.as_node().or_else(|| elem.parent())?;
    SyntaxAncestors::start_from(node).find_map(ast::ScopedName::cast).or_else(|| {
        let prev = root.token_before_offset(offset)?;
        SyntaxAncestors::start_from(prev.parent).find_map(ast::ScopedName::cast)
    })
}

fn members_for_incomplete_scoped_access(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<Vec<Member>> {
    let separator = root.token_before_offset(offset)?;
    if separator.kind() != syntax::Token![::] {
        return None;
    }
    let left = root.token_before_offset(separator.text_range()?.start())?;
    let res = sema.nameres_ident(file_id, left, NameContext::Type);
    let members = TypeSystem::new(db.db, db.resolution())
        .members(&TypeSystem::new(db.db, db.resolution()).type_of_resolution(res));
    (!members.is_empty()).then_some(members)
}

fn members_for_incomplete_access(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<Vec<Member>> {
    let dot = root.token_before_offset(offset)?;
    if dot.kind() != syntax::Token![.] {
        return None;
    }

    let dot_start = dot.text_range()?.start();
    let expr = expr_before_dot(dot.parent, dot_start)?;

    members_for_expr(db, sema, file_id, expr)
}

fn expr_before_dot(
    parent: SyntaxNode<'_>,
    dot_start: utils::text_edit::TextSize,
) -> Option<ast::Expression<'_>> {
    parent
        .children()
        .filter_map(|elem| elem.as_node())
        .filter_map(ast::Expression::cast)
        .find(|expr| expr.syntax().text_range().is_some_and(|r| r.end() == dot_start))
}

fn members_for_expr(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    expr: ast::Expression<'_>,
) -> Option<Vec<Member>> {
    let expr_id = sema.resolve_expr(file_id, expr)?;
    let types = TypeSystem::new(db.db, db.resolution());
    let mut members = types.members(&types.type_of_expr(expr_id));
    if members.is_empty() {
        members = types.members(&types.type_of_resolution(sema.expr_to_def(expr_id)));
    }
    (!members.is_empty()).then_some(members)
}

fn members_for_scoped_name(
    db: &AnalysisContext<'_>,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    scoped: ast::ScopedName<'_>,
) -> Option<Vec<Member>> {
    if let Some(left) = scoped_left_token(scoped) {
        let res = sema.nameres_ident(file_id, left, NameContext::Type);
        let types = TypeSystem::new(db.db, db.resolution());
        let members = types.members(&types.type_of_resolution(res));
        return (!members.is_empty()).then_some(members);
    }

    let left = ast::Expression::cast(scoped.left().syntax())?;
    members_for_expr(db, sema, file_id, left)
}

fn scoped_left_token(scoped: ast::ScopedName<'_>) -> Option<SyntaxTokenWithParent<'_>> {
    use ast::Name::*;
    match scoped.left() {
        IdentifierName(ident) => {
            Some(SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        IdentifierSelectName(ident) => {
            Some(SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        _ => None,
    }
}
