use hir_def::symbol::DefKind;
use hir_semantics::semantics::Semantics;
use syntax::ast;

use super::candidate::CompletionCandidate;
use crate::{
    FilePosition,
    completion::{context::CompletionContext, request::PortListKind},
    db::root_db::RootDb,
};

pub(super) fn complete_in_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
    kind: PortListKind,
) -> Vec<CompletionCandidate> {
    match kind {
        PortListKind::Ansi => complete_ansi_port_list(db, position, prefix, ctx),
        PortListKind::Function => complete_function_port_list(db, position, prefix, ctx),
        PortListKind::NonAnsi => complete_non_ansi_port_list(db, position, prefix, ctx),
    }
}

fn complete_ansi_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    visible_typedefs_in_module_header(db, position)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionCandidate::text(name, ctx.replacement))
        .collect()
}

fn complete_function_port_list(
    db: &RootDb,
    position: FilePosition,
    prefix: &str,
    ctx: &CompletionContext,
) -> Vec<CompletionCandidate> {
    visible_typedefs_in_module_header(db, position)
        .into_iter()
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionCandidate::text(name, ctx.replacement))
        .collect()
}

fn visible_typedefs_in_module_header(db: &RootDb, position: FilePosition) -> Vec<String> {
    let sema = Semantics::new(db);
    let file_id = position.file_id.into();
    let parsed_file = sema.parse_file(position.file_id);
    let Some(root) = parsed_file.root() else {
        return Vec::new();
    };
    let module = sema.find_node_at_offset::<ast::ModuleDeclaration>(root, position.offset);
    let Some(module) = module else {
        return Vec::new();
    };
    let Some(module_id) = sema.module_to_def(file_id, module) else {
        return Vec::new();
    };

    let unit_scope = db.unit_scope();
    let module_scope = db.scope(module_id);
    let mut names: Vec<String> =
        unit_scope.typedef_names(db).map(|ident| ident.to_string()).collect();

    names.extend(module_scope.typedef_names(db).map(|ident| ident.to_string()));

    names.sort();
    names.dedup();
    names
}

fn complete_non_ansi_port_list(
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
    let module = sema.find_node_at_offset::<ast::ModuleDeclaration>(root, position.offset);
    let Some(module) = module else {
        return Vec::new();
    };
    let Some(module_id) = sema.module_to_def(file_id, module) else {
        return Vec::new();
    };
    let scope = db.scope(module_id);
    scope
        .iter_listing()
        .filter_map(|(ident, defs)| {
            defs.iter()
                .any(|def_id| matches!(def_id.kind(db), DefKind::Port | DefKind::NonAnsiPort))
                .then(|| ident.to_string())
        })
        .filter(|name| name.starts_with(prefix))
        .map(|name| CompletionCandidate::text(name, ctx.replacement))
        .collect()
}
