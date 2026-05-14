use hir::{
    db::HirDb,
    hir_def::{
        Ident,
        block::BlockId,
        module::{ModuleId, instantiation::InstanceId},
    },
    scope::UnitEntry,
    semantics::{Semantics, pathres::PathResolution},
};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::{get::GetRef, text_edit::TextEditItem};

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
    let root = sema.parse_root(position.file_id);

    let scope = member_access_at_offset(root, position.offset)
        .and_then(|access| resolve_scope_for_expr(db, &sema, access.left()))
        .or_else(|| resolve_scope_for_incomplete_access(db, &sema, root, position.offset));
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
    let prev = root.token_before_offset(offset)?;
    if prev.kind() != syntax::Token![.] {
        return None;
    }
    SyntaxAncestors::start_from(prev.parent).find_map(ast::MemberAccessExpression::cast)
}

fn resolve_scope_for_incomplete_access(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    root: SyntaxNode<'_>,
    offset: utils::text_edit::TextSize,
) -> Option<MemberScope> {
    let dot = root.token_before_offset(offset)?;
    if dot.kind() != syntax::Token![.] {
        return None;
    }

    let dot_start = dot.text_range()?.start();
    let expr = expr_before_dot(dot.parent, dot_start)?;

    resolve_scope_for_expr(db, sema, expr)
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

fn resolve_scope_for_expr(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    expr: ast::Expression<'_>,
) -> Option<MemberScope> {
    let res = sema.expr_to_def(sema.resolve_expr(expr))?;
    resolve_scope_from_resolution(db, res)
}

fn resolve_scope_from_resolution(db: &RootDb, res: PathResolution) -> Option<MemberScope> {
    match res {
        PathResolution::Module(module_id) => Some(MemberScope::Module(module_id)),
        PathResolution::Instance(instance) => {
            resolve_instance_target_module_id(db, instance.module_id, instance.value)
                .map(MemberScope::Module)
        }
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

fn scope_member_names(db: &RootDb, scope: MemberScope) -> Vec<Ident> {
    let mut names: Vec<Ident> = match scope {
        MemberScope::Module(module_id) => {
            db.module_scope(module_id).iter().map(|(name, _)| name.clone()).collect()
        }
        MemberScope::Block(block_id) => {
            db.block_scope(block_id).iter().map(|(name, _)| name.clone()).collect()
        }
    };

    names.sort();
    names.dedup();
    names
}
