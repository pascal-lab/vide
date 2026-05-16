use hir::{db::HirDb, hir_def::lower_ident_opt, semantics::Semantics};
use ide_db::root_db::RootDb;
use rustc_hash::FxHashSet;
use span::FilePosition;
use syntax::ast::{self, AstNode};
use utils::text_edit::TextEditItem;

use super::{
    CompletionItem, CompletionItemKind,
    instantiation::{
        enclosing_instantiation, overridable_params_of_module_sorted, ports_of_module_sorted,
    },
    typed_filter::{
        const_candidates_in_module, expected_param_ty, expected_port_ty, is_compatible_typed_value,
        value_candidates_in_module,
    },
};
use crate::completion::context::CompletionContext;

pub(super) fn complete_named_port_names(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let Some(root) = sema.parse_root(position.file_id) else {
        return Vec::new();
    };
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(root, position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let mut used_named_ports = FxHashSet::default();
    if let Some(instance) =
        sema.find_node_at_offset::<ast::HierarchicalInstance>(root, position.offset)
    {
        for conn in instance.connections().children() {
            if let Some(named) = conn.as_named_port_connection()
                && let Some(name) = lower_ident_opt(named.name())
            {
                used_named_ports.insert(name);
            }
        }
    }

    ports_of_module_sorted(db, target_module_id)
        .into_iter()
        .filter(|name| name.as_str().starts_with(prefix))
        .filter(|name| !used_named_ports.contains(name))
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
    let Some(root) = sema.parse_root(position.file_id) else {
        return Vec::new();
    };
    let Some(instantiation) =
        sema.find_node_at_offset::<ast::HierarchyInstantiation>(root, position.offset)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let mut used_named_params = FxHashSet::default();
    if let Some(params) = instantiation.parameters() {
        for assignment in params.parameters().children() {
            if let Some(named) = assignment.as_named_param_assignment()
                && let Some(name) = lower_ident_opt(named.name())
            {
                used_named_params.insert(name);
            }
        }
    }

    overridable_params_of_module_sorted(db, target_module_id)
        .into_iter()
        .filter(|name| name.as_str().starts_with(prefix))
        .filter(|name| !used_named_params.contains(name))
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
    let Some(root) = sema.parse_root(position.file_id) else {
        return Vec::new();
    };
    let Some(conn) = sema.find_node_at_offset::<ast::NamedPortConnection>(root, position.offset)
    else {
        return Vec::new();
    };

    let Some(port_name) = hir::hir_def::lower_ident_opt(conn.name()) else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(conn.syntax()) else {
        return Vec::new();
    };

    let Some(current_module_id) = sema.resolve_instantiation(instantiation).map(|it| it.module_id)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let Some(expected_ty) = expected_port_ty(db, &target_module, target_module_id, &port_name)
    else {
        return Vec::new();
    };

    let current_module = db.module(current_module_id);
    let candidates = value_candidates_in_module(db, current_module_id);

    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| {
            is_compatible_typed_value(
                db,
                &target_module,
                expected_ty,
                &current_module,
                *candidate_ty,
            )
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
    let Some(root) = sema.parse_root(position.file_id) else {
        return Vec::new();
    };
    let Some(assign) = sema.find_node_at_offset::<ast::NamedParamAssignment>(root, position.offset)
    else {
        return Vec::new();
    };

    let Some(param_name) = hir::hir_def::lower_ident_opt(assign.name()) else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(assign.syntax()) else {
        return Vec::new();
    };

    let Some(current_module_id) = sema.resolve_instantiation(instantiation).map(|it| it.module_id)
    else {
        return Vec::new();
    };
    let Some(target_module_id) = sema.nameres_instantiation(instantiation) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let Some(expected_ty) = expected_param_ty(db, &target_module, target_module_id, &param_name)
    else {
        return Vec::new();
    };

    let current_module = db.module(current_module_id);
    let candidates = const_candidates_in_module(db, current_module_id);

    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| {
            is_compatible_typed_value(
                db,
                &target_module,
                expected_ty,
                &current_module,
                *candidate_ty,
            )
        })
        .map(|(name, _)| CompletionItem {
            label: name.clone(),
            kind: CompletionItemKind::Text,
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
            snippet_edit: None,
        })
        .collect()
}
