use hir_def::{
    Ident,
    db::HirDefDb,
    module::ModuleId,
    symbol::{DefKind, NameContext, Resolution},
};
use hir_ty::{Compatibility, Type, TypeSystem};

use crate::db::root_db::RootDb;

pub(super) fn expected_port_ty(
    db: &RootDb,
    target_module_id: ModuleId,
    port_name: &Ident,
) -> Option<Type> {
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
    Some(TypeSystem::new(db).type_of_resolution(res))
}

pub(super) fn expected_param_ty(
    db: &RootDb,
    target_module_id: ModuleId,
    param_name: &Ident,
) -> Option<Type> {
    let res =
        crate::module_resolution::resolve_named_param_in_module(db, target_module_id, param_name);
    if res.is_unresolved() {
        return None;
    }
    Some(TypeSystem::new(db).type_of_resolution(res))
}

pub(super) fn value_candidates_in_module(db: &RootDb, module_id: ModuleId) -> Vec<(String, Type)> {
    typed_candidates_in_module(db, module_id, |kind| {
        matches!(
            kind,
            DefKind::Variable
                | DefKind::Net
                | DefKind::Genvar
                | DefKind::Specparam
                | DefKind::Port
                | DefKind::NonAnsiPort
        )
    })
}

pub(super) fn const_candidates_in_module(db: &RootDb, module_id: ModuleId) -> Vec<(String, Type)> {
    typed_candidates_in_module(db, module_id, |kind| kind == DefKind::Param)
}

pub(super) fn is_compatible_typed_value(db: &RootDb, expected: &Type, candidate: &Type) -> bool {
    TypeSystem::new(db).compatibility(expected, candidate) == Compatibility::Compatible
}

fn typed_candidates_in_module(
    db: &RootDb,
    module_id: ModuleId,
    include: impl Fn(DefKind) -> bool,
) -> Vec<(String, Type)> {
    let types = TypeSystem::new(db);
    let mut candidates: Vec<_> = db
        .module_scope(module_id)
        .iter_listing()
        .filter_map(|(name, defs)| {
            let resolution =
                Resolution::from_candidates(defs.into_iter().filter(|def| include(def.kind(db))));
            (!resolution.is_unresolved())
                .then(|| (name.to_string(), types.type_of_resolution(resolution)))
        })
        .collect();
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates
}
