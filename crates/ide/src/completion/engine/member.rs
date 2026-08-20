use std::ops::Range;

use base_db::source_db::SourceDb;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};

use super::candidate::CompletionCandidate;
use crate::{
    FilePosition, analysis::AnalysisContext, completion::context::CompletionContext, slang_class,
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
        return members_to_candidates(
            slang_class::list_scope_members_at(db, position.file_id, &name),
            prefix,
            ctx,
        );
    }

    let Some(expr) = dot_prefix_expr(root, position.offset) else {
        return Vec::new();
    };
    let Some(name) = expr_source_text(db, position.file_id, expr) else {
        return Vec::new();
    };
    let named = slang_class::list_scope_members_at(db, position.file_id, &name);
    if matches!(&named, crate::elaboration::ElabResult::Ready(Some(members)) if !members.is_empty())
    {
        return members_to_candidates(named, prefix, ctx);
    }
    let Some(range) = expr.syntax().text_range() else {
        return members_to_candidates(named, prefix, ctx);
    };
    members_to_candidates(
        slang_class::list_members_at(
            db,
            position.file_id,
            usize::from(range.end()).saturating_sub(1),
        ),
        prefix,
        ctx,
    )
}

fn members_to_candidates(
    result: crate::elaboration::ElabResult<Vec<slang_sys::compilation::MemberInfo>>,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    let crate::elaboration::ElabResult::Ready(Some(members)) = result else {
        return Vec::new();
    };
    members
        .into_iter()
        .filter(|member| member.name.starts_with(prefix))
        .map(|member| CompletionCandidate::text(member.name, ctx.replacement))
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

fn dot_prefix_expr(
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<ast::Expression<'_>> {
    if let Some(access) = member_access_at_offset(root, offset) {
        return Some(access.left());
    }
    let prev = root.token_before_offset(offset)?;
    if prev.kind() != syntax::Token![.] {
        return None;
    }
    expr_before_dot(prev.parent, prev.text_range()?.start())
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

fn expr_source_text(
    db: &AnalysisContext<'_>,
    file_id: vfs::FileId,
    expr: ast::Expression<'_>,
) -> Option<String> {
    let range = expr.syntax().text_range()?;
    let text = db.file_text(file_id);
    Some(text.get(Range::<usize>::from(range))?.trim().to_owned())
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

fn scoped_left_token(scoped: ast::ScopedName<'_>) -> Option<syntax::SyntaxTokenWithParent<'_>> {
    use ast::Name::*;
    match scoped.left() {
        IdentifierName(ident) => {
            Some(syntax::SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        IdentifierSelectName(ident) => {
            Some(syntax::SyntaxTokenWithParent { parent: ident.syntax(), tok: ident.identifier()? })
        }
        _ => None,
    }
}
