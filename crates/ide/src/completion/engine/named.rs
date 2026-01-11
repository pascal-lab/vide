use hir::{db::HirDb, hir_def::module::ModuleId, semantics::Semantics};
use ide_db::root_db::RootDb;
use span::FilePosition;
use syntax::ast::{self, AstNode};
use utils::{
    get::{Get, GetRef},
    text_edit::TextEditItem,
};

use super::typed_filter::{
    const_candidates_in_module, expected_param_ty, expected_port_ty, is_compatible_typed_value,
    value_candidates_in_module,
};
use crate::completion::context::CompletionContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub edit: Option<TextEditItem>,
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

    ports_of_module(db, target_module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
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

    overridable_params_of_module(db, target_module_id)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionItem {
            label: name.clone(),
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
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
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
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
            edit: Some(TextEditItem::replace(ctx.replacement, name)),
        })
        .collect()
}

fn ports_of_module(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let module = db.module(module_id);
    let mut names = Vec::new();

    match &module.ports {
        hir::hir_def::module::port::Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        names.push(name.to_string());
                    }
                }
            }
        }
        hir::hir_def::module::port::Ports::NonAnsi { ports, .. } => {
            for (_, port) in ports.iter() {
                if let Some(label) = port.label.as_ref() {
                    names.push(label.to_string());
                }
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

fn overridable_params_of_module(db: &RootDb, module_id: ModuleId) -> Vec<String> {
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);

    let mut names = Vec::new();

    for (_decl_id, decl) in module.decls.iter() {
        if decl.name.is_none() {
            continue;
        }
        let hir::hir_def::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) =
            decl.parent
        else {
            continue;
        };
        let hir::hir_def::declaration::Declaration::ParamDecl(_) = module.get(declaration_id)
        else {
            continue;
        };

        let src = module_src_map.get(declaration_id);
        let hir::hir_def::declaration::DeclarationSrc::ParameterDeclaration(ptr) = src else {
            continue;
        };
        let Some(ast_decl) = ptr.to_node(&tree).and_then(ast::ParameterDeclaration::cast) else {
            continue;
        };

        let Some(keyword) = ast_decl.keyword() else {
            continue;
        };
        if keyword.kind() != syntax::Token![parameter] {
            continue;
        }

        names.push(decl.name.as_ref().unwrap().to_string());
    }

    names.sort();
    names.dedup();
    names
}

fn enclosing_instantiation(node: syntax::SyntaxNode) -> Option<ast::HierarchyInstantiation> {
    syntax::SyntaxAncestors::start_from(node).find_map(ast::HierarchyInstantiation::cast)
}
