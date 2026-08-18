use hir_def::{
    Ident,
    aggregate::{StructId, StructKind},
    container::OwnerRef,
    def_id::DefId,
    expr::data_ty::DataTy,
    owner::OwnerId,
    symbol::Resolution,
};

use crate::{
    db::TyDb,
    infer::{apply_unpacked_dimensions, normalize_data_ty, type_of_path_resolution_impl},
    ty::{Ty, TyMember, TyResult},
};

pub(crate) fn members_of_ty(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    ty: &Ty,
) -> Vec<TyMember> {
    match ty {
        Ty::Alias { target, .. } => members_of_ty(db, context, target),
        Ty::Struct(struct_id) => struct_members(db, context, *struct_id),
        Ty::Union(def_id) => union_members(db, context, *def_id),
        Ty::Module(module_id) => module_members(db, context, *module_id),
        Ty::Checker(def_id) => checker_members(db, context, *def_id),
        Ty::Covergroup(def_id) => covergroup_members(db, context, *def_id),
        Ty::VirtualInterface { def, .. } => def
            .primary_origin(db)
            .as_module(db)
            .map(|module_id| module_members(db, context, module_id))
            .unwrap_or_default(),
        Ty::GenerateBlock(generate_block_id) => {
            generate_block_members(db, context, *generate_block_id)
        }
        Ty::Block(block_id) => block_members(db, context, *block_id),
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

pub(crate) fn select_member(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    base: &Ty,
    name: &Ident,
) -> TyResult {
    members_of_ty(db, context, base)
        .into_iter()
        .find(|member| &member.name == name)
        .map(|member| TyResult::new(member.ty))
        .unwrap_or_else(|| TyResult::new(Ty::Unknown))
}

fn struct_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    struct_id: OwnerRef<StructId>,
) -> Vec<TyMember> {
    let data = struct_id.cont_id.data(db);
    data.struct_def(struct_id.value)
        .members
        .iter()
        .filter_map(|member| {
            let name = member.name.clone()?;
            let ty = member
                .ty
                .as_ref()
                .map(|ty| {
                    let normalized =
                        normalize_data_ty(db, context, ty.cont_id, ty.value.clone()).ty;
                    apply_unpacked_dimensions(
                        db,
                        context,
                        ty.cont_id,
                        normalized,
                        &member.dimensions,
                    )
                })
                .unwrap_or(Ty::Unknown);
            Some(TyMember { name, ty })
        })
        .collect()
}

fn union_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    def_id: DefId,
) -> Vec<TyMember> {
    aggregate_struct_id_from_def(db, def_id)
        .filter(|struct_id| struct_kind(db, *struct_id) == StructKind::Union)
        .map(|struct_id| struct_members(db, context, struct_id))
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
fn module_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    module_id: OwnerId,
) -> Vec<TyMember> {
    let is_package = module_id.module_kind(db) == Some(hir_def::module::ModuleKind::Package);
    if is_package {
        let exports = db.package_exports(context, module_id);
        scope_members(db, context, exports.iter_listing())
    } else {
        let scope = db.scope(module_id);
        scope_members(db, context, scope.iter_listing())
    }
}

fn checker_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    def_id: DefId,
) -> Vec<TyMember> {
    let scope = db.scope(def_id.container_id(db));
    scope_members(db, context, scope.iter_listing())
}

fn covergroup_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    def_id: DefId,
) -> Vec<TyMember> {
    let scope = db.scope(def_id.container_id(db));
    scope_members(db, context, scope.iter_listing())
}

fn generate_block_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    generate_block_owner: OwnerId,
) -> Vec<TyMember> {
    let scope = db.scope(generate_block_owner);
    scope_members(db, context, scope.iter_listing())
}

fn block_members(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    owner: hir_def::owner::OwnerId,
) -> Vec<TyMember> {
    let scope = db.scope(owner);
    scope_members(db, context, scope.iter_listing())
}

fn scope_members<'a, I, D>(
    db: &dyn TyDb,
    context: &hir_def::pathres::ResolutionContext,
    entries: I,
) -> Vec<TyMember>
where
    I: Iterator<Item = (&'a Ident, D)>,
    D: IntoIterator<Item = DefId>,
{
    let mut members: Vec<_> = entries
        .map(|(name, defs)| {
            let resolution = Resolution::from_candidates(defs);
            let ty = type_of_path_resolution_impl(db, context, resolution).ty;
            TyMember { name: name.clone(), ty }
        })
        .collect();
    members.sort_by(|left, right| left.name.cmp(&right.name));
    members.dedup_by(|left, right| left.name == right.name);
    members
}
