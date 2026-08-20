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
        let members = slang_class::list_scope_members_at(db, position.file_id, &name)
            .answered("scope member completion")
            .unwrap_or_default();
        return to_candidates(members, prefix, ctx);
    }

    let Some(expr) = dot_prefix_expr(root, position.offset) else {
        return Vec::new();
    };
    let Some(range) = expr.syntax().text_range() else {
        return Vec::new();
    };
    // A prefix reaches members two ways and the spelling does not say which:
    // `top.u0` and `u0[0]` name instance bodies, `pkt` is a variable whose
    // struct type has the fields. Only slang can tell them apart, so it is
    // asked as a name first and as an expression second.
    //
    // Collapsing this needs the offset resolver to see expressions, not just
    // declarations: `FindAtOffset` visits symbols, so a *use* of `top.u0`
    // has no symbol at that range. That in turn needs the buffer to parse,
    // and a buffer being completed in does not. Two questions, not a guess.
    let file_text = db.file_text(position.file_id);
    let Some(prefix_text) = file_text.get(Range::<usize>::from(range)).map(str::trim) else {
        return Vec::new();
    };
    let by_name = slang_class::list_scope_members_at(db, position.file_id, prefix_text)
        .answered("member completion by name")
        .unwrap_or_default();
    if !by_name.is_empty() {
        return to_candidates(by_name, prefix, ctx);
    }
    let by_type =
        slang_class::list_members_at(db, position.file_id, usize::from(range.end()).saturating_sub(1))
            .answered("member completion by type")
            .unwrap_or_default();
    to_candidates(by_type, prefix, ctx)
}

fn to_candidates(
    members: Vec<slang_sys::compilation::MemberInfo>,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
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
