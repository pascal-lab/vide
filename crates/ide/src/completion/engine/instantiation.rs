use hir::{
    db::HirDb,
    hir_def::{
        Ident,
        declaration::{Declaration, DeclarationSrc},
        expr::declarator::DeclaratorParent,
        module::{ModuleId, port::Ports},
    },
};
use syntax::{
    SyntaxAncestors,
    ast::{self, AstNode},
};
use utils::get::{Get, GetRef};

use crate::db::root_db::RootDb;

pub(super) fn ports_of_module_sorted(db: &RootDb, module_id: ModuleId) -> Vec<Ident> {
    let mut names = ports_of_module_in_order(db, module_id);
    names.sort();
    names.dedup();
    names
}

pub(super) fn ports_of_module_in_order(db: &RootDb, module_id: ModuleId) -> Vec<Ident> {
    let module = db.module(module_id);
    let mut names = Vec::new();

    match &module.ports {
        Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        names.push(name.clone());
                    }
                }
            }
        }
        Ports::NonAnsi { ports, .. } => {
            for (_, port) in ports.iter() {
                if let Some(label) = port.label.as_ref() {
                    names.push(label.clone());
                }
            }
        }
    }

    names
}

pub(super) fn overridable_params_of_module_sorted(db: &RootDb, module_id: ModuleId) -> Vec<Ident> {
    let mut names = overridable_params_of_module_in_order(db, module_id);
    names.sort();
    names.dedup();
    names
}

pub(super) fn overridable_params_of_module_in_order(
    db: &RootDb,
    module_id: ModuleId,
) -> Vec<Ident> {
    let (module, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);

    let mut names = Vec::new();

    for (_decl_id, decl) in module.decls.iter() {
        if decl.name.is_none() {
            continue;
        }
        let DeclaratorParent::DeclarationId(declaration_id) = decl.parent else {
            continue;
        };
        let Declaration::ParamDecl(_) = module.get(declaration_id) else {
            continue;
        };

        let Some(DeclarationSrc::ParameterDeclaration(ptr)) = module_src_map.get(declaration_id)
        else {
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

        if let Some(name) = decl.name.as_ref() {
            names.push(name.clone());
        }
    }

    names
}

pub(super) fn enclosing_instantiation(
    node: syntax::SyntaxNode,
) -> Option<ast::HierarchyInstantiation> {
    SyntaxAncestors::start_from(node).find_map(ast::HierarchyInstantiation::cast)
}
