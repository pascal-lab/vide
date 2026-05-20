use hir::{
    file::HirFileId,
    semantics::Semantics,
    type_infer::{TyMember, members_of_ty, type_of_expr, type_of_path_resolution},
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use super::candidate::CompletionCandidate;
use crate::completion::context::CompletionContext;

pub(super) fn complete_member_access(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let sema = Semantics::new(db);
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
        .map(|member| member.name)
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
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<Vec<TyMember>> {
    let separator = root.token_before_offset(offset)?;
    if separator.kind() != syntax::Token![::] {
        return None;
    }
    let left = root.token_before_offset(separator.text_range()?.start())?;
    let res = sema.nameres_ident(file_id, left)?;
    let members = members_of_ty(db, &type_of_path_resolution(db, res).ty);
    (!members.is_empty()).then_some(members)
}

fn members_for_incomplete_access(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<Vec<TyMember>> {
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
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    expr: ast::Expression<'_>,
) -> Option<Vec<TyMember>> {
    let ty = type_of_expr(db, sema.resolve_expr(file_id, expr)?).ty;
    let members = members_of_ty(db, &ty);
    (!members.is_empty()).then_some(members)
}

fn members_for_scoped_name(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    file_id: HirFileId,
    scoped: ast::ScopedName<'_>,
) -> Option<Vec<TyMember>> {
    let left = ast::Expression::cast(scoped.left().syntax())?;
    members_for_expr(db, sema, file_id, left)
}
