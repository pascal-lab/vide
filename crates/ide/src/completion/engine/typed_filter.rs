use hir::{
    container::InContainer,
    db::HirDb,
    hir_def::{Ident, declaration::Declaration, module::ModuleId},
    scope::ModuleEntry,
    semantics::pathres::PathResolution,
    type_infer::{
        Ty, TyClass, packed_bit_width, type_class, type_of_decl, type_of_path_resolution,
    },
};
use ide_db::root_db::RootDb;
use utils::get::{Get, GetRef};

pub(super) fn expected_port_ty(
    db: &RootDb,
    target_module_id: ModuleId,
    port_name: &Ident,
) -> Option<Ty> {
    let scope = db.module_scope(target_module_id);
    let entry = scope.get(port_name)?;

    match entry {
        ModuleEntry::AnsiPortEntry(_) | ModuleEntry::NonAnsiPortEntry(_) => Some(
            type_of_path_resolution(
                db,
                PathResolution::from(hir::container::InModule::new(target_module_id, entry)),
            )
            .ty,
        ),
        _ => None,
    }
}

pub(super) fn expected_param_ty(
    db: &RootDb,
    target_module_id: ModuleId,
    param_name: &Ident,
) -> Option<Ty> {
    let target_module = db.module(target_module_id);
    let scope = db.module_scope(target_module_id);
    let ModuleEntry::DeclId(decl_id) = scope.get(param_name)? else {
        return None;
    };

    let hir::hir_def::expr::declarator::DeclaratorParent::DeclarationId(declaration_id) =
        target_module.get(decl_id).parent
    else {
        return None;
    };
    let Declaration::ParamDecl(_) = target_module.get(declaration_id) else {
        return None;
    };

    is_overridable_parameter_decl(db, target_module_id, declaration_id)
        .then(|| type_of_decl(db, InContainer::new(target_module_id.into(), decl_id)).ty)
}

pub(super) fn value_candidates_in_module(db: &RootDb, module_id: ModuleId) -> Vec<(String, Ty)> {
    let module = db.module(module_id);
    let mut candidates: Vec<(String, Ty)> = Vec::new();

    for (_, decl) in module.declarations.iter() {
        match decl {
            Declaration::DataDecl(_)
            | Declaration::NetDecl(_)
            | Declaration::GenvarDecl(_)
            | Declaration::SpecparamDecl(_) => {
                for decl_id in decl.decls().clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        let ty = type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty;
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
            Declaration::ParamDecl(_) => {}
        }
    }

    match &module.ports {
        hir::hir_def::module::port::Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        let ty = type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty;
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
        hir::hir_def::module::port::Ports::NonAnsi { decls, .. } => {
            for (_, port_decl) in decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        let ty = type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty;
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.dedup_by(|a, b| a.0 == b.0);
    candidates
}

pub(super) fn const_candidates_in_module(db: &RootDb, module_id: ModuleId) -> Vec<(String, Ty)> {
    let module = db.module(module_id);
    let mut candidates: Vec<(String, Ty)> = Vec::new();

    for (_, decl) in module.declarations.iter() {
        let Declaration::ParamDecl(param_decl) = decl else {
            continue;
        };
        for decl_id in param_decl.decls.clone() {
            if let Some(name) = module.get(decl_id).name.as_ref() {
                let ty = type_of_decl(db, InContainer::new(module_id.into(), decl_id)).ty;
                candidates.push((name.to_string(), ty));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    candidates.dedup_by(|a, b| a.0 == b.0);
    candidates
}

pub(super) fn is_compatible_typed_value(db: &RootDb, expected: &Ty, candidate: &Ty) -> bool {
    let (Some(expected_class), Some(candidate_class)) =
        (type_class(db, expected), type_class(db, candidate))
    else {
        return false;
    };
    if expected_class != candidate_class {
        return false;
    }

    if expected_class != TyClass::Integral {
        return true;
    }

    match (packed_bit_width(db, expected), packed_bit_width(db, candidate)) {
        (Some(expected), Some(candidate)) => expected == candidate,
        _ => false,
    }
}

fn is_overridable_parameter_decl(
    db: &RootDb,
    module_id: ModuleId,
    declaration_id: hir::hir_def::declaration::DeclarationId,
) -> bool {
    let (_, module_src_map) = db.module_with_source_map(module_id);
    let tree = db.parse(module_id.file_id);
    let Some(hir::hir_def::declaration::DeclarationSrc::ParameterDeclaration(ptr)) =
        module_src_map.get(declaration_id)
    else {
        return false;
    };
    let Some(node) = ptr.to_node(&tree) else {
        return false;
    };
    node.first_token().is_some_and(|kw| kw.kind() == syntax::Token![parameter])
}
