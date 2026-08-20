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
    let parsed_file = db.semantics().parse_file(position.file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };
    if let Some(name) = colon_colon_scope_name(root, position.offset) {
        let crate::elaboration::ElabResult::Ready(Some(members)) =
            crate::slang_class::list_scope_members_at(db, position.file_id, &name)
        else {
            return Vec::new();
        };
        return members
            .into_iter()
            .filter(|member| member.name.starts_with(prefix))
            .map(|member| CompletionCandidate::text(member.name, ctx.replacement))
            .collect();
    }

    let sema = db.semantics();
    let file_id = position.file_id.into();
    let members = member_access_at_offset(root, position.offset)
        .and_then(|access| members_for_expr(db, &sema, file_id, access.left()))
        .or_else(|| members_for_incomplete_access(db, &sema, file_id, root, position.offset));
    let Some(members) = members else {
        return Vec::new();
    };
    members
        .into_iter()
        .map(Member::into_name)
        .filter(|name| name.as_str().starts_with(prefix))
        .map(|name| CompletionCandidate::text(name.to_string(), ctx.replacement))
        .collect()
}

fn colon_colon_scope_name(
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<String> {
    let prev = root.token_before_offset(offset)?;
    if prev.kind() == syntax::Token![::] {
        let left = root.token_before_offset(prev.text_range()?.start())?;
        return Some(left.tok.raw_text().to_string());
    }
    let scoped = scoped_name_at_offset(root, offset)?;
    if scoped_uses_dot(scoped) {
        return None;
    }
    let left = scoped_left_token(scoped)?;
    Some(left.tok.raw_text().to_string())
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

fn scoped_uses_dot(scoped: ast::ScopedName<'_>) -> bool {
    scoped
        .syntax()
        .children()
        .filter_map(|elem| elem.as_token())
        .any(|tok| tok.kind() == syntax::Token![.])
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
