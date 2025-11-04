use rustc_hash::FxHashMap;
use utils::get::GetRef;

use super::{Source2DefCtx, pathres::PathResolution};
use crate::{
    container::{ContainerId, ContainerParent, InBlock, InContainer, InModule},
    hir_def::{
        Ident,
        expr::{Expr, ExprId},
    },
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
            ContainerId::SubroutineId(_) => None,
            ContainerId::FileSubroutineId(_) => None,
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
            ContainerId::BlockId(block_id) => {
                let scope = db.block_scope(block_id);
                let entry = scope.get(&ident)?;
                Some(InBlock::new(block_id, entry).into())
            }
            ContainerId::PackageId(_) => None,
            ContainerId::SubroutineId(_) => None,
            ContainerId::FileSubroutineId(_) => None,
        })?;
        self.hir_cache.name_map.insert(InContainer::new(cont_id, ident), res);
        Some(res)
    }
}
