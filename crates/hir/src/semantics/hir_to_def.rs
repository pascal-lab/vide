use rustc_hash::FxHashMap;
use utils::get::GetRef;

use super::{Source2DefCtx, pathres::PathResolution};
use crate::{
    container::{ContainerId, ContainerParent, InBlock, InContainer, InModule, InSubroutine},
    hir_def::{
        Ident,
        block::BlockId,
        expr::{Expr, ExprId},
        module::{ModuleId, instantiation::InstanceId},
    },
    scope::UnitEntry,
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
                let field = field.as_ref()?;
                let receiver_res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
                let res = self.resolve_member_from_resolution(receiver_res, field)?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res);
                Some(res)
            }
            Expr::ElementSelect { receiver, .. } => {
                let res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
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
            ContainerId::BlockId(block_id) => {
                let block = db.block(block_id);
                resolve(block.get(expr_id))
            }
            ContainerId::SubroutineId(_) => None,
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
            ContainerId::SubroutineId(subroutine_id) => {
                let scope = db.subroutine_scope(subroutine_id);
                let entry = scope.get(&ident)?;
                Some(InSubroutine::new(subroutine_id, entry).into())
            }
        })?;
        self.hir_cache.name_map.insert(InContainer::new(cont_id, ident), res);
        Some(res)
    }

    fn resolve_member_from_resolution(
        &mut self,
        res: PathResolution,
        field: &Ident,
    ) -> Option<PathResolution> {
        match res {
            PathResolution::Module(module_id) => self.resolve_member_in_module(module_id, field),
            PathResolution::Instance(instance) => {
                let target_module =
                    self.instance_target_module_id(instance.module_id, instance.value)?;
                self.resolve_member_in_module(target_module, field)
            }
            PathResolution::Block(block_id) => self.resolve_member_in_block(block_id, field),
            _ => None,
        }
    }

    fn resolve_member_in_module(
        &mut self,
        module_id: ModuleId,
        field: &Ident,
    ) -> Option<PathResolution> {
        let scope = self.db.module_scope(module_id);
        let entry = scope.get(field)?;
        Some(InModule::new(module_id, entry).into())
    }

    fn resolve_member_in_block(
        &mut self,
        block_id: BlockId,
        field: &Ident,
    ) -> Option<PathResolution> {
        let scope = self.db.block_scope(block_id);
        let entry = scope.get(field)?;
        Some(InBlock::new(block_id, entry).into())
    }

    fn instance_target_module_id(
        &mut self,
        module_id: ModuleId,
        instance_id: InstanceId,
    ) -> Option<ModuleId> {
        let module = self.db.module(module_id);
        let instance = module.get(instance_id);
        let instantiation = module.get(instance.parent);
        let module_name = instantiation.module_name.as_ref()?;
        match self.db.unit_scope().get(module_name)? {
            UnitEntry::ModuleId(module_id) => Some(module_id),
            _ => None,
        }
    }
}
