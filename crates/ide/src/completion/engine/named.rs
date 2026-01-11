use hir::{db::HirDb, semantics::Semantics};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::ast::{self, AstNode};
use utils::text_edit::TextEditItem;

use super::{
    instantiation::{
        enclosing_instantiation, overridable_params_of_module_sorted, ports_of_module_sorted,
    },
    typed_filter::{
        const_candidates_in_module, expected_param_ty, expected_port_ty, is_compatible_typed_value,
        value_candidates_in_module,
    },
};
use crate::completion::context::CompletionContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionItemKind,
    pub edit: Option<TextEditItem>,
    pub snippet_edit: Option<TextEditItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionItemKind {
    Text,
    Keyword,
    Snippet,
}

pub(super) fn complete_named_port_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    ports_of_module_sorted(db, target_module_id)
        .into_iter()
        .filter(|name| name.as_str().starts_with(prefix))
        .map(|name| {
            let label = name.to_string();
            let plain = format!("{label}()");
            let snippet = format!("{label}(${{1:expr}})");
            CompletionItem {
                label: label.clone(),
                kind: CompletionItemKind::Text,
                edit: Some(TextEditItem::replace(ctx.replacement, plain)),
                snippet_edit: Some(TextEditItem::replace(ctx.replacement, snippet)),
            }
        })
        .collect()
}

pub(super) fn complete_named_param_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    overridable_params_of_module_sorted(db, target_module_id)
        .into_iter()
        .filter(|name| name.as_str().starts_with(prefix))
        .map(|name| {
            let label = name.to_string();
            let plain = format!("{label}()");
            let snippet = format!("{label}(${{1:expr}})");
            CompletionItem {
                label: label.clone(),
                kind: CompletionItemKind::Text,
                edit: Some(TextEditItem::replace(ctx.replacement, plain)),
                snippet_edit: Some(TextEditItem::replace(ctx.replacement, snippet)),
            }
        })
        .collect()
}

pub(super) fn complete_named_port_conn_expr(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(conn) =
        sema.find_node_at_offset::<ast::NamedPortConnection>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };

    let Some(port_name) = hir::hir_def::lower_ident_opt(conn.name()) else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(conn.syntax()) else {
        return Vec::new();
    };

    let current_module_id = sema.resolve_instantiation(instantiation).module_id;
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let expected_ty = expected_port_ty(db, &target_module, target_module_id, &port_name);

    let current_module = db.module(current_module_id);
    let candidates = value_candidates_in_module(db, current_module_id);

    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| {
            expected_ty.is_none_or(|expected_ty| {
                is_compatible_typed_value(
                    db,
                    &target_module,
                    expected_ty,
                    &current_module,
                    *candidate_ty,
                )
            })
        })
        .map(|(name, _)| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect()
}

pub(super) fn complete_named_param_assign_expr(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let Some(assign) =
        sema.find_node_at_offset::<ast::NamedParamAssignment>(file.syntax(), position.offset)
    else {
        return Vec::new();
    };

    let Some(param_name) = hir::hir_def::lower_ident_opt(assign.name()) else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(assign.syntax()) else {
        return Vec::new();
    };

    let current_module_id = sema.resolve_instantiation(instantiation).module_id;
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let expected_ty = expected_param_ty(db, &target_module, target_module_id, &param_name);

    let current_module = db.module(current_module_id);
    let candidates = const_candidates_in_module(db, current_module_id);

    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| {
            expected_ty.is_none_or(|expected_ty| {
                is_compatible_typed_value(
                    db,
                    &target_module,
                    expected_ty,
                    &current_module,
                    *candidate_ty,
                )
            })
        })
        .map(|(name, _)| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect()
}
