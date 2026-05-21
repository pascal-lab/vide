use hir::{db::HirDb, hir_def::lower_ident_opt, semantics::Semantics};
use ide_db::root_db::RootDb;
use rustc_hash::FxHashSet;
use span::FilePosition;
use syntax::{
    SyntaxAncestors, SyntaxNodeExt,
    ast::{self, AstNode},
    has_text_range::HasTextRange,
};
use utils::get::Get;

use super::{
    candidate::CompletionCandidate,
    expr,
    instantiation::{
        enclosing_instantiation, overridable_params_of_module_in_order,
        overridable_params_of_module_sorted, ports_of_module_in_order, ports_of_module_sorted,
    },
    typed_filter::{
        const_candidates_in_module, expected_param_ty, expected_port_ty, is_compatible_typed_value,
        value_candidates_in_module,
    },
};
use crate::{
    completion::{
        context::CompletionContext,
        request::{HashKind, ParenListKind},
    },
    module_resolution::resolve_instantiation_target,
};

pub(super) fn complete_in_paren_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    kind: ParenListKind,
) -> Vec<CompletionCandidate> {
    match kind {
        ParenListKind::PortConnections => complete_port_connections(db, position, prefix, ctx),
        ParenListKind::ParamValueAssignment => {
            complete_param_value_assignment(db, position, prefix, ctx)
        }
        ParenListKind::ParameterPortList => {
            complete_parameter_port_list_with_typedefs(db, position, prefix, ctx)
        }
        ParenListKind::Arguments => expr::complete_argument_exprs(db, position, prefix, ctx),
    }
}

pub(super) fn complete_after_hash(
    _prefix: &str,
    ctx: &CompletionContext,
    kind: HashKind,
) -> Vec<CompletionCandidate> {
    let (label, snippet_label) = match kind {
        HashKind::ParamValueAssignment => ("#(...)", "params"),
        HashKind::ParameterPortList => ("#(parameter ...)", "parameter ..."),
    };

    vec![CompletionCandidate::snippet(
        label,
        ctx.replacement,
        "()",
        format!("(${{1:{snippet_label}}})"),
    )]
}

fn complete_parameter_port_list_with_typedefs(
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
    let Some(module) = sema.find_node_at_offset::<ast::ModuleDeclaration>(root, position.offset)
    else {
        return Vec::new();
    };
    let (_, file_src_map) = db.hir_file_with_source_map(file_id);
    let module_src = hir::hir_def::module::ModuleSrc::from(module);
    let Some(module_id) =
        file_src_map.get(module_src).map(|id| hir::hir_def::module::ModuleId::new(file_id, id))
    else {
        return Vec::new();
    };

    let mut items: Vec<CompletionCandidate> = db
        .unit_scope()
        .iter()
        .filter_map(|(ident, entry)| {
            matches!(entry, hir::scope::UnitEntry::FiledTypedefId(_)).then_some(ident)
        })
        .chain(db.module_scope(module_id).iter().filter_map(|(ident, entry)| {
            matches!(entry, hir::scope::ModuleEntry::TypedefId(_)).then_some(ident)
        }))
        .map(|ident| ident.to_string())
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionCandidate::text(name, ctx.replacement))
        .collect();

    items.sort_by(|a, b| a.label().cmp(b.label()));
    items.dedup_by(|a, b| a.label() == b.label());
    items
}

fn complete_port_connections(
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
    let Some(current_module_id) =
        sema.resolve_instantiation(file_id, instantiation).map(|it| it.module_id)
    else {
        return Vec::new();
    };
    let Some(target_module_id) =
        resolve_target_module_id(db, &sema, position.file_id, instantiation)
    else {
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
                CompletionCandidate::text_snippet(label, ctx.replacement, plain, snippet)
            })
            .collect();
    }

    let index = separated_list_index_at_offset(instance.connections(), position.offset);
    let ports = ports_of_module_in_order(db, target_module_id);
    let Some(port_name) = ports.get(index) else {
        return Vec::new();
    };

    let Some(expected_ty) = expected_port_ty(db, target_module_id, port_name) else {
        return Vec::new();
    };

    let candidates = value_candidates_in_module(db, current_module_id);
    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| is_compatible_typed_value(db, &expected_ty, candidate_ty))
        .map(|(name, _)| CompletionCandidate::text(name, ctx.replacement))
        .collect()
}

fn complete_param_value_assignment(
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

    let elem = root.covering_element(utils::line_index::TextRange::empty(position.offset));
    let Some(node) = elem.as_node().or_else(|| elem.parent()) else {
        return Vec::new();
    };

    let Some(instantiation) =
        SyntaxAncestors::start_from(node).find_map(ast::HierarchyInstantiation::cast)
    else {
        return Vec::new();
    };

    let Some(current_module_id) =
        sema.resolve_instantiation(file_id, instantiation).map(|it| it.module_id)
    else {
        return Vec::new();
    };
    let Some(target_module_id) =
        resolve_target_module_id(db, &sema, position.file_id, instantiation)
    else {
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
                CompletionCandidate::text_snippet(label, ctx.replacement, plain, snippet)
            })
            .collect();
    }

    let index = separated_list_index_at_offset(params.parameters(), position.offset);
    let params_in_order = overridable_params_of_module_in_order(db, target_module_id);
    let Some(param_name) = params_in_order.get(index) else {
        return Vec::new();
    };

    let Some(expected_ty) = expected_param_ty(db, target_module_id, param_name) else {
        return Vec::new();
    };

    let candidates = const_candidates_in_module(db, current_module_id);
    candidates
        .into_iter()
        .filter(|(name, _)| name.starts_with(prefix))
        .filter(|(_, candidate_ty)| is_compatible_typed_value(db, &expected_ty, candidate_ty))
        .map(|(name, _)| CompletionCandidate::text(name, ctx.replacement))
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
    _sema: &Semantics<'_, RootDb>,
    from_file: vfs::FileId,
    instantiation: ast::HierarchyInstantiation<'_>,
) -> Option<hir::hir_def::module::ModuleId> {
    resolve_instantiation_target(db, from_file, instantiation).unique()
}
