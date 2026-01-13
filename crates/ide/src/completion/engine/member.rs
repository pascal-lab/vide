use hir::{
    db::HirDb,
    hir_def::{
        Ident, block::BlockId,
        module::{ModuleId, instantiation::InstanceId},
    },
    scope::UnitEntry,
    semantics::{Semantics, pathres::PathResolution},
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxCursorExt, SyntaxNode, SyntaxToken, SyntaxTokenWithParent,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::get::GetRef;
use utils::text_edit::TextEditItem;

use super::{CompletionItem, CompletionItemKind};
use crate::completion::context::CompletionContext;

#[derive(Debug, Clone, Copy)]
enum MemberScope {
    Module(ModuleId),
    Block(BlockId),
}

pub(super) fn complete_member_access(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let root = file.syntax();

    let scope = sema
        .find_node_at_offset::<ast::MemberAccessExpression>(root, position.offset)
        .or_else(|| member_access_at_offset(root, position.offset))
        .and_then(|access| resolve_scope_for_expr(db, &sema, access.left()))
        .or_else(|| resolve_scope_before_dot(db, &sema, root, position.offset));
    let Some(scope) = scope else {
        return Vec::new();
    };

    scope_member_names(db, scope)
        .into_iter()
        .filter(|name| name.as_str().starts_with(prefix))
        .map(|name| {
            let label = name.to_string();
            CompletionItem {
                label: label.clone(),
                kind: CompletionItemKind::Text,
                edit: Some(TextEditItem::replace(ctx.replacement, label)),
                snippet_edit: None,
            }
        })
        .collect()
}

fn member_access_at_offset(
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<ast::MemberAccessExpression<'_>> {
    let prev = token_before_offset(root, offset)?;
    if prev.kind() != syntax::Token![.] {
        return None;
    }
    SyntaxAncestors::start_from(prev.parent).find_map(ast::MemberAccessExpression::cast)
}

fn token_before_offset(
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<SyntaxTokenWithParent<'_>> {
    let mut cursor = root.walk();
    if !cursor.goto_last_tok_before(offset) {
        return None;
    }
    cursor.to_tok_with_parent()
}

fn resolve_scope_before_dot(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<MemberScope> {
    let dot = token_before_offset(root, offset)?;
    if dot.kind() != syntax::Token![.] {
        return None;
    }
    let dot_range = dot.text_range()?;
    let mut cursor = root.walk();
    if !cursor.goto_last_tok_before(dot_range.start()) {
        return None;
    }
    let prev = cursor.to_tok_with_parent()?;

    if let Some(access) = SyntaxAncestors::start_from(prev.parent)
        .find_map(ast::MemberAccessExpression::cast)
        .filter(|access| access.name() == Some(prev.tok))
    {
        let expr = ast::Expression::cast(access.syntax())?;
        return resolve_scope_for_expr(db, sema, expr);
    }

    if let Some(scoped) = SyntaxAncestors::start_from(prev.parent)
        .find_map(ast::ScopedName::cast)
        .filter(|scoped| scoped_right_token(*scoped) == Some(prev.tok))
    {
        let expr = ast::Expression::cast(scoped.syntax())?;
        return resolve_scope_for_expr(db, sema, expr);
    }

    if let Some(expr) = SyntaxAncestors::start_from(prev.parent).find_map(ast::Expression::cast) {
        return resolve_scope_for_expr(db, sema, expr);
    }

    let name = SyntaxAncestors::start_from(prev.parent).find_map(ast::Name::cast)?;
    let expr = ast::Expression::cast(name.syntax())?;
    resolve_scope_for_expr(db, sema, expr)
}

fn resolve_scope_for_expr(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    expr: ast::Expression<'_>,
) -> Option<MemberScope> {
    let res = sema.expr_to_def(sema.resolve_expr(expr))?;
    resolve_scope_from_resolution(db, res)
}

fn resolve_scope_from_resolution(
    db: &RootDb,
    res: PathResolution,
) -> Option<MemberScope> {
    match res {
        PathResolution::Module(module_id) => Some(MemberScope::Module(module_id)),
        PathResolution::Instance(instance) => resolve_instance_target_module_id(
            db,
            instance.module_id,
            instance.value,
        )
        .map(MemberScope::Module),
        PathResolution::Block(block_id) => Some(MemberScope::Block(block_id)),
        _ => None,
    }
}

fn resolve_instance_target_module_id(
    db: &RootDb,
    module_id: ModuleId,
    instance_id: InstanceId,
) -> Option<ModuleId> {
    let module = db.module(module_id);
    let instance = module.get(instance_id);
    let instantiation = module.get(instance.parent);
    let module_name = instantiation.module_name.as_ref()?;
    match db.unit_scope().get(module_name)? {
        UnitEntry::ModuleId(module_id) => Some(module_id),
        _ => None,
    }
}

fn scoped_right_token(scoped: ast::ScopedName<'_>) -> Option<SyntaxToken<'_>> {
    use ast::Name::*;
    match scoped.right() {
        IdentifierName(ident) => ident.identifier(),
        IdentifierSelectName(ident) => ident.identifier(),
        _ => None,
    }
}

fn scope_member_names(db: &RootDb, scope: MemberScope) -> Vec<Ident> {
    let mut names: Vec<Ident> = match scope {
        MemberScope::Module(module_id) => db
            .module_scope(module_id)
            .iter()
            .map(|(name, _)| name.clone())
            .collect(),
        MemberScope::Block(block_id) => db
            .block_scope(block_id)
            .iter()
            .map(|(name, _)| name.clone())
            .collect(),
    };

    names.sort();
    names.dedup();
    names
}
