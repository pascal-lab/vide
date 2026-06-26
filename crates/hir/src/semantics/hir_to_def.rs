use rustc_hash::FxHashMap;
use utils::get::GetRef;

use super::{Source2DefCtx, pathres::PathResolution};
use crate::{
    container::{InContainer, ScopeId},
    hir_def::{
        Ident,
        block::BlockId,
        expr::{Expr, ExprId},
        module::{ModuleId, generate::GenerateBlockId, instantiation::InstanceId},
    },
    semantics::pathres::{name_scope, resolve_name},
    symbol::{DefLoc, NameContext},
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
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res.clone());
                Some(res)
            }
            Expr::ElementSelect { receiver, .. } => {
                let res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res.clone());
                Some(res)
            }
            Expr::Ident(ident) => {
                let res =
                    self.name_to_def(InContainer::new(cont_id, ident.clone()), NameContext::Value)?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res.clone());
                Some(res)
            }
            _ => None,
        };

        match cont_id {
            ScopeId::File(file_id) => {
                let file = db.hir_file(file_id);
                resolve(file.get(expr_id))
            }
            ScopeId::Module(in_file) => {
                let module = db.module(in_file);
                resolve(module.get(expr_id))
            }
            ScopeId::Block(block_id) => {
                let block = db.block(block_id);
                resolve(block.get(expr_id))
            }
            ScopeId::GenerateBlock(generate_block_id) => {
                let generate_block = db.generate_block(generate_block_id);
                resolve(generate_block.get(expr_id))
            }
            ScopeId::Subroutine(subroutine_id) => {
                let subroutine = db.subroutine(subroutine_id);
                resolve(subroutine.get(expr_id))
            }
        }
    }

    pub(super) fn name_to_def(
        &mut self,
        InContainer { cont_id, value: ident }: InContainer<Ident>,
        name_ctx: NameContext,
    ) -> Option<PathResolution> {
        let res = resolve_name(self.db, cont_id, &ident, name_ctx)?;
        self.hir_cache.name_map.insert(InContainer::new(cont_id, ident), res.clone());
        Some(res)
    }

    fn resolve_member_from_resolution(
        &mut self,
        res: PathResolution,
        field: &Ident,
    ) -> Option<PathResolution> {
        for def_id in res.def_ids() {
            match def_id.loc(self.db) {
                DefLoc::Module(module_id) => {
                    if let Some(res) = self.resolve_member_in_module(module_id, field) {
                        return Some(res);
                    }
                }
                DefLoc::Instance(instance) => {
                    let target_module =
                        self.instance_target_module_id(instance.module_id, instance.value)?;
                    if let Some(res) = self.resolve_member_in_module(target_module, field) {
                        return Some(res);
                    }
                }
                DefLoc::Block(block_id) => {
                    if let Some(res) = self.resolve_member_in_block(block_id, field) {
                        return Some(res);
                    }
                }
                DefLoc::GenerateBlock(generate_block_id) => {
                    if let Some(res) =
                        self.resolve_member_in_generate_block(generate_block_id, field)
                    {
                        return Some(res);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn resolve_member_in_module(
        &mut self,
        module_id: ModuleId,
        field: &Ident,
    ) -> Option<PathResolution> {
        name_scope(self.db, module_id.into())
            .lookup(NameContext::Value, field)
            .and_then(PathResolution::from_def_ids)
    }

    fn resolve_member_in_block(
        &mut self,
        block_id: BlockId,
        field: &Ident,
    ) -> Option<PathResolution> {
        name_scope(self.db, block_id.into())
            .lookup(NameContext::Value, field)
            .and_then(PathResolution::from_def_ids)
    }

    fn resolve_member_in_generate_block(
        &mut self,
        generate_block_id: GenerateBlockId,
        field: &Ident,
    ) -> Option<PathResolution> {
        name_scope(self.db, generate_block_id.into())
            .lookup(NameContext::Value, field)
            .and_then(PathResolution::from_def_ids)
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
        self.db.unit_scope().module_ids(self.db, module_name).unique()
    }
}
