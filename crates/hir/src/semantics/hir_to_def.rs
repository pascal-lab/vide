use rustc_hash::FxHashMap;
use utils::get::GetRef;

use super::{Source2DefCtx, pathres::PathResolution};
use crate::{
    container::{
        ContainerId, ContainerParent, InBlock, InContainer, InModule, InPackage, InSubroutine,
    },
    hir_def::{
        Ident,
        expr::{Expr, ExprId},
    },
    scope::PackageEntry,
};

#[derive(Default, Debug)]
pub(super) struct Hir2DefCache {
    expr_map: FxHashMap<InContainer<ExprId>, PathResolution>,
    name_map: FxHashMap<InContainer<Ident>, PathResolution>,
}

impl Source2DefCtx<'_, '_> {
    pub(super) fn expr_to_def(
        &mut self,
        InContainer { cont_id, value: expr_id }: InContainer<ExprId>,
    ) -> Option<PathResolution> {
        let db = self.db;

        let mut resolve = |expr: &Expr| match expr {
            Expr::Field { receiver, field } => {
                let field_ident = field.clone()?;
                let receiver_res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
                let res = match receiver_res {
                    PathResolution::Package(package_id) => {
                        let package_scope = db.package_scope(package_id);
                        let entry = package_scope.get(&field_ident)?;
                        match entry {
                            PackageEntry::DeclId(in_pkg_decl) => {
                                PathResolution::Decl(in_pkg_decl.into())
                            }
                            PackageEntry::TypedefId(in_pkg_typedef) => {
                                PathResolution::Typedef(in_pkg_typedef.into())
                            }
                            PackageEntry::ClassId(in_pkg_class) => {
                                PathResolution::Class(in_pkg_class.into())
                            }
                            PackageEntry::StructId(_)
                            | PackageEntry::ProcId(_)
                            | PackageEntry::SubroutineId(_) => return None,
                            PackageEntry::Package(in_pkg_pkg) => {
                                PathResolution::Package(in_pkg_pkg.value)
                            }
                        }
                    }
                    _ => return None,
                };
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res);
                Some(res)
            }
            Expr::Ident(ident) => {
                let res = self.name_to_def(InContainer::new(cont_id, ident.clone()))?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res);
                Some(res)
            }
            _ => None,
        };

        match cont_id {
            ContainerId::HirFileId(file_id) => {
                let file = db.hir_file(file_id);
                resolve(file.get(expr_id))
            }
            ContainerId::ModuleId(in_file) => {
                let module = db.module(in_file);
                resolve(module.get(expr_id))
            }
            ContainerId::PackageId(package_id) => {
                let package = db.package(package_id);
                resolve(package.get(expr_id))
            }
            ContainerId::BlockId(block_id) => {
                let block = db.block(block_id);
                resolve(block.get(expr_id))
            }
            ContainerId::SubroutineId(loc) => {
                let subroutine = db.subroutine(loc);
                resolve(&subroutine.exprs[expr_id])
            }
        }
    }

    pub(super) fn name_to_def(
        &mut self,
        InContainer { cont_id, value: ident }: InContainer<Ident>,
    ) -> Option<PathResolution> {
        let db = self.db;
        let res = ContainerParent::start_from(db, cont_id).find_map(|id| match id {
            ContainerId::HirFileId(_) => {
                let scope = db.unit_scope();
                let entry = scope.get(&ident)?;
                Some(entry.into())
            }
            ContainerId::ModuleId(module_id) => {
                let scope = db.module_scope(module_id);
                let entry = scope.get(&ident)?;
                Some(InModule::new(module_id, entry).into())
            }
            ContainerId::PackageId(package_id) => {
                let scope = db.package_scope(package_id);
                let entry = scope.get(&ident)?;
                Some(InPackage::new(package_id, entry).into())
            }
            ContainerId::BlockId(block_id) => {
                let scope = db.block_scope(block_id);
                let entry = scope.get(&ident)?;
                Some(InBlock::new(block_id, entry).into())
            }
            ContainerId::SubroutineId(loc) => {
                let scope = db.subroutine_scope(loc);
                let entry = scope.get(&ident)?;
                Some(InSubroutine::new(loc, entry).into())
            }
        })?;
        self.hir_cache.name_map.insert(InContainer::new(cont_id, ident), res);
        Some(res)
    }
}
