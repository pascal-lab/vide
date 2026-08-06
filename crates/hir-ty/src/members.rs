use hir_def::{
    Ident,
    aggregate::{StructId, StructKind},
    container::InContainer,
    def_id::DefId,
    expr::data_ty::DataTy,
    module::{ModuleId, ModuleKind, generate::GenerateBlockId},
    symbol::{DefOriginLoc, NameScope, Resolution},
};
use utils::get::GetRef;

use crate::{
    db::TyDb,
    infer::{data_ty_of_decl, normalize_data_ty, type_of_path_resolution_impl},
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

fn struct_members(db: &dyn TyDb, struct_id: InContainer<StructId>) -> Vec<TyMember> {
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

fn aggregate_struct_id_from_def(db: &dyn TyDb, def_id: DefId) -> Option<InContainer<StructId>> {
    let data_ty = match def_id.primary_origin(db).loc(db) {
        DefOriginLoc::Typedef(typedef) => {
            typedef.cont_id.data(db).typedef(typedef.value).ty.clone()?
        }
        DefOriginLoc::Decl(decl) => data_ty_of_decl(db, decl.clone())?,
        _ => return None,
    };
    match data_ty {
        DataTy::Struct(struct_id) => Some(struct_id),
        _ => None,
    }
}

fn struct_kind(db: &dyn TyDb, struct_id: InContainer<StructId>) -> StructKind {
    struct_id.cont_id.data(db).struct_def(struct_id.value).kind
}

fn module_members(db: &dyn TyDb, module_id: ModuleId) -> Vec<TyMember> {
    let file = db.hir_file(module_id.file_id);
    let scope = if file.get(module_id.value).kind == ModuleKind::Package {
        db.package_export_scope(module_id)
    } else {
        db.module_scope(module_id)
    };
    scope_members(db, &scope)
}

fn checker_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    let Some(checker_id) = def_id.primary_origin(db).as_checker(db) else {
        return Vec::new();
    };
    scope_members(db, &db.checker_scope(checker_id))
}

fn covergroup_members(db: &dyn TyDb, def_id: DefId) -> Vec<TyMember> {
    let Some(covergroup_id) = def_id.primary_origin(db).as_covergroup(db) else {
        return Vec::new();
    };
    scope_members(db, &db.covergroup_scope(covergroup_id))
}

fn generate_block_members(db: &dyn TyDb, generate_block_id: GenerateBlockId) -> Vec<TyMember> {
    scope_members(db, &db.generate_block_scope(generate_block_id))
}

fn block_members(db: &dyn TyDb, block_id: hir_def::block::BlockId) -> Vec<TyMember> {
    scope_members(db, &db.block_scope(block_id))
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
