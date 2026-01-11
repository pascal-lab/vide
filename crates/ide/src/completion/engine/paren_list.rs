use hir::{db::HirDb, hir_def::lower_ident_opt, semantics::Semantics};
use ide_db::root_db::RootDb;
use rustc_hash::FxHashSet;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::text_edit::TextEditItem;

use super::{
    CompletionItem, CompletionItemKind,
    instantiation::{
        enclosing_instantiation, overridable_params_of_module_in_order,
        overridable_params_of_module_sorted, ports_of_module_in_order, ports_of_module_sorted,
    },
    typed_filter::{
        const_candidates_in_module, expected_param_ty, expected_port_ty, is_compatible_typed_value,
        value_candidates_in_module,
    },
};
use crate::completion::context::{CompletionContext, ParenListKind};

pub(super) fn complete_in_paren_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    kind: ParenListKind,
) -> Vec<CompletionItem> {
    match kind {
        ParenListKind::PortConnections => complete_port_connections(db, position, prefix, ctx),
        ParenListKind::ParamValueAssignment => {
            complete_param_value_assignment(db, position, prefix, ctx)
        }
        _ => Vec::new(),
    }
}

fn complete_port_connections(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let root = file.syntax();

    let elem = root.covering_element(utils::line_index::TextRange::empty(position.offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return Vec::new();
    };

    let Some(instance) =
        SyntaxAncestors::start_from(node).find_map(ast::HierarchicalInstance::cast)
    else {
        return Vec::new();
    };

    let Some(instantiation) = enclosing_instantiation(instance.syntax()) else {
        return Vec::new();
    };
    let current_module_id = sema.resolve_instantiation(instantiation).module_id;
    let Some(target_module_id) = resolve_target_module_id(db, &sema, instantiation) else {
        return Vec::new();
    };

    let mut has_named = false;
    let mut has_ordered = false;
    let mut used_named_ports: FxHashSet<hir::hir_def::Ident> = FxHashSet::default();
    for conn in instance.connections().children() {
        if let Some(named) = conn.as_named_port_connection() {
            has_named = true;
            if let Some(name) = lower_ident_opt(named.name()) {
                used_named_ports.insert(name);
            }
        }
        has_ordered |= conn.as_ordered_port_connection().is_some();
    }

    if has_named || !has_ordered {
        return ports_of_module_sorted(db, target_module_id)
            .into_iter()
            .filter(|name| name.as_str().starts_with(prefix))
            .filter(|name| !used_named_ports.contains(name))
            .map(|name| {
                let label = name.to_string();
                let plain = format!(".{label}()");
                let snippet = format!(".{label}(${{1:expr}})");
                CompletionItem {
                    label,
                    kind: CompletionItemKind::Text,
                    edit: Some(TextEditItem::replace(ctx.replacement, plain)),
                    snippet_edit: Some(TextEditItem::replace(ctx.replacement, snippet)),
                }
            })
            .collect();
    }

    let index = separated_list_index_at_offset(instance.connections(), position.offset);
    let ports = ports_of_module_in_order(db, target_module_id);
    let Some(port_name) = ports.get(index) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let expected_ty = expected_port_ty(db, &target_module, target_module_id, port_name);

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

fn complete_param_value_assignment(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionItem> {
    let sema = Semantics::new(db);
    let file = sema.parse(position.file_id);
    let root = file.syntax();

    let elem = root.covering_element(utils::line_index::TextRange::empty(position.offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return Vec::new();
    };

    let Some(instantiation) =
        SyntaxAncestors::start_from(node).find_map(ast::HierarchyInstantiation::cast)
    else {
        return Vec::new();
    };

    let current_module_id = sema.resolve_instantiation(instantiation).module_id;
    let Some(target_module_id) = resolve_target_module_id(db, &sema, instantiation) else {
        return Vec::new();
    };
    let Some(params) = instantiation.parameters() else {
        return Vec::new();
    };

    let mut has_named = false;
    let mut has_ordered = false;
    let mut used_named_params: FxHashSet<hir::hir_def::Ident> = FxHashSet::default();
    for assignment in params.parameters().children() {
        if let Some(named) = assignment.as_named_param_assignment() {
            has_named = true;
            if let Some(name) = lower_ident_opt(named.name()) {
                used_named_params.insert(name);
            }
        }
        has_ordered |= assignment.as_ordered_param_assignment().is_some();
    }

    if has_named || !has_ordered {
        return overridable_params_of_module_sorted(db, target_module_id)
            .into_iter()
            .filter(|name| name.as_str().starts_with(prefix))
            .filter(|name| !used_named_params.contains(name))
            .map(|name| {
                let label = name.to_string();
                let plain = format!(".{label}()");
                let snippet = format!(".{label}(${{1:expr}})");
                CompletionItem {
                    label,
                    kind: CompletionItemKind::Text,
                    edit: Some(TextEditItem::replace(ctx.replacement, plain)),
                    snippet_edit: Some(TextEditItem::replace(ctx.replacement, snippet)),
                }
            })
            .collect();
    }

    let index = separated_list_index_at_offset(params.parameters(), position.offset);
    let params_in_order = overridable_params_of_module_in_order(db, target_module_id);
    let Some(param_name) = params_in_order.get(index) else {
        return Vec::new();
    };

    let target_module = db.module(target_module_id);
    let expected_ty = expected_param_ty(db, &target_module, target_module_id, param_name);

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

fn separated_list_index_at_offset<'a, T: AstNode<'a>>(
    list: ast::SeparatedList<'a, T>,
    offset: utils::line_index::TextSize,
) -> usize {
    let mut idx = 0usize;
    for item in list.children() {
        let Some(range) = item.syntax().text_range() else {
            continue;
        };
        if range.is_empty() && range.start() == offset {
            return idx;
        }

        if !range.is_empty() && (range.contains(offset) || range.end() == offset) {
            return idx;
        }

        if range.end() < offset {
            idx += 1;
        } else {
            break;
        }
    }
    idx
}

fn resolve_target_module_id(
    db: &RootDb,
    sema: &Semantics<'_, RootDb>,
    instantiation: ast::HierarchyInstantiation<'_>,
) -> Option<hir::hir_def::module::ModuleId> {
    if let Some(module_id) = sema.nameres_instantiation(instantiation) {
        return Some(module_id);
    }

    let name = hir::hir_def::lower_ident_opt(instantiation.type_())?;
    match db.unit_scope().get(&name)? {
        hir::scope::UnitEntry::ModuleId(module_id) => Some(module_id),
        _ => None,
    }
}
