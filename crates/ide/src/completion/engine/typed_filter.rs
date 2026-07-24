use hir::{
    container::InContainer,
    db::HirDb,
    hir_def::{
        Ident,
        declaration::Declaration,
        module::{ModuleId, port::Ports},
    },
    symbol::{NameContext, Resolution},
    type_infer::{Ty, TyClass, packed_bit_width, type_class},
};
use utils::get::GetRef;

use crate::db::root_db::RootDb;

pub(super) fn expected_port_ty(
    db: &RootDb,
    target_module_id: ModuleId,
    port_name: &Ident,
) -> Option<Ty> {
    let scope = db.module_scope(target_module_id);
    let res = Resolution::from_candidates(
        scope
            .lookup(NameContext::Value, port_name)
            .into_candidates()
            .into_iter()
            .filter(|def_id| def_id.is_port(db)),
    );
    if res.is_unresolved() {
        return None;
    }
    Some(db.type_of_path_resolution(res).ty.clone())
}

pub(super) fn expected_param_ty(
    db: &RootDb,
    target_module_id: ModuleId,
    param_name: &Ident,
) -> Option<Ty> {
    let res =
        crate::module_resolution::resolve_named_param_in_module(db, target_module_id, param_name);
    if res.is_unresolved() {
        return None;
    }
    Some(db.type_of_path_resolution(res).ty.clone())
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
                        let ty =
                            db.type_of_decl(InContainer::new(module_id.into(), decl_id)).ty.clone();
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
            Declaration::ParamDecl(_) => {}
        }
    }

    match &module.ports {
        Ports::Ansi(port_decls) => {
            for (_, port_decl) in port_decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        let ty =
                            db.type_of_decl(InContainer::new(module_id.into(), decl_id)).ty.clone();
                        candidates.push((name.to_string(), ty));
                    }
                }
            }
        }
        Ports::NonAnsi { decls, .. } => {
            for (_, port_decl) in decls.iter() {
                for decl_id in port_decl.decls.clone() {
                    if let Some(name) = module.get(decl_id).name.as_ref() {
                        let ty =
                            db.type_of_decl(InContainer::new(module_id.into(), decl_id)).ty.clone();
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
                let ty = db.type_of_decl(InContainer::new(module_id.into(), decl_id)).ty.clone();
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
