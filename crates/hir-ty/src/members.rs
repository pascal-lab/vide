use hir_def::{
    Ident,
    aggregate::{StructId, StructKind},
    container::OwnerRef,
    def_id::DefId,
    expr::data_ty::DataTy,
    owner::OwnerId,
    symbol::{NameScope, Resolution},
};

use crate::{
    db::TyDb,
    infer::{normalize_data_ty, type_of_path_resolution_impl},
    ty::{Ty, TyMember, TyResult},
};

pub(crate) fn members_of_ty(db: &dyn TyDb, ty: &Ty) -> Vec<TyMember> {
    match ty {
        Ty::Alias { target, .. } => members_of_ty(db, target),
        Ty::Struct(struct_id) => struct_members(db, struct_id.clone()),
        Ty::Union(def_id) => union_members(db, def_id.clone()),
        Ty::Module(module_id) => module_members(db, *module_id),
        Ty::Checker(def_id) => checker_members(db, def_id.clone()),
        Ty::Covergroup(def_id) => covergroup_members(db, def_id.clone()),
        Ty::VirtualInterface { def, .. } => def
            .primary_origin(db)
            .as_module(db)
            .map(|module_id| module_members(db, module_id))
            .unwrap_or_default(),
        Ty::GenerateBlock(generate_block_id) => {
            generate_block_members(db, generate_block_id.clone())
        }
        Ty::Block(block_id) => block_members(db, block_id.clone()),
        Ty::Unknown
        | Ty::Error
        | Ty::Void
        | Ty::Builtin(_)
        | Ty::Enum(_)
        | Ty::Queue { .. }
        | Ty::Assoc { .. }
        | Ty::Dynamic(_)
        | Ty::Event
        | Ty::Chandle => Vec::new(),
    }
}

pub(crate) fn select_member(db: &dyn TyDb, base: &Ty, name: &Ident) -> TyResult {
    members_of_ty(db, base)
        .into_iter()
        .find(|member| &member.name == name)
        .map(|member| TyResult::new(member.ty))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn struct_members(db: &dyn TyDb, struct_id: OwnerRef<StructId>) -> Vec<TyMember> {
    let data = struct_id.cont_id.data(db);
    data.struct_def(struct_id.value)
        .members
        .iter()
        .filter_map(|member| {
            let name = member.name.clone()?;
            let ty = member
                .ty
                .as_ref()
                .map(|ty| normalize_data_ty(db, ty.cont_id.clone(), ty.value.clone()).ty)
                .unwrap_or(Ty::Unknown);
            Some(TyMember { name, ty })
        })
        .collect()
}

fn union_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    aggregate_struct_id_from_def(db, def_id)
        .filter(|struct_id| struct_kind(db, struct_id.clone()) == StructKind::Union)
        .map(|struct_id| struct_members(db, struct_id))
        .unwrap_or_default()
}

fn aggregate_struct_id_from_def(db: &dyn TyDb, def_id: DefId) -> Option<OwnerRef<StructId>> {
    match def_id.data_type(db)? {
        DataTy::Struct(struct_id) => Some(struct_id),
        _ => None,
    }
}

fn struct_kind(db: &dyn TyDb, struct_id: OwnerRef<StructId>) -> StructKind {
    struct_id.cont_id.data(db).struct_def(struct_id.value).kind
}
fn module_members(db: &dyn TyDb, module_id: OwnerId) -> Vec<TyMember> {
    let is_package = module_id.module_kind(db) == Some(hir_def::module::ModuleKind::Package);
    let scope =
        if is_package { db.package_export_scope(module_id) } else { db.scope_for(module_id) };
    scope_members(db, &scope)
}

fn checker_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    scope_members(db, &db.scope_for(def_id.container_id(db)))
}

fn covergroup_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    scope_members(db, &db.scope_for(def_id.container_id(db)))
}

fn generate_block_members(db: &dyn TyDb, generate_block_owner: OwnerId) -> Vec<TyMember> {
    scope_members(db, &db.scope_for(generate_block_owner))
}

fn block_members(db: &dyn TyDb, owner: hir_def::owner::OwnerId) -> Vec<TyMember> {
    scope_members(db, &db.scope_for(owner))
}

fn scope_members(db: &dyn TyDb, scope: &NameScope) -> Vec<TyMember> {
    let mut members: Vec<_> = scope
        .iter_listing()
        .map(|(name, defs)| {
            let resolution = Resolution::from_candidates(defs);
            let ty = type_of_path_resolution_impl(db, resolution).ty;
            TyMember { name: name.clone(), ty }
        })
        .collect();
    members.sort_by(|left, right| left.name.cmp(&right.name));
    members.dedup_by(|left, right| left.name == right.name);
    members
}
