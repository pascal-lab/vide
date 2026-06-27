use rustc_hash::FxHashMap;
use utils::get::GetRef;

use super::{Source2DefCtx, pathres::PathResolution};
use crate::{
    container::{InContainer, ScopeId},
    hir_def::{
        Ident,
        expr::{Expr, ExprId},
    },
    semantics::pathres::{descend_scope, name_scope, resolve_name, resolve_path},
    symbol::NameContext,
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
                if let Some(res) = self.resolve_expr_path(cont_id, expr_id, NameContext::Value) {
                    self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res.clone());
                    return Some(res);
                }
                let receiver_res = self.expr_to_def(InContainer::new(cont_id, *receiver))?;
                let res = self.resolve_member_from_resolution(receiver_res, field)?;
                self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res.clone());
                Some(res)
            }
            Expr::ElementSelect { receiver, .. } => {
                if let Some(res) = self.resolve_expr_path(cont_id, expr_id, NameContext::Value) {
                    self.hir_cache.expr_map.insert(InContainer::new(cont_id, expr_id), res.clone());
                    return Some(res);
                }
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
                let subroutine = db.subroutine(subroutine_id.as_in_container());
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
            let Some(scope_id) = descend_scope(self.db, *def_id) else {
                continue;
            };
            if let Some(res) = name_scope(self.db, scope_id)
                .lookup(NameContext::Value, field)
                .and_then(PathResolution::from_def_ids)
            {
                return Some(res);
            }
        }
        None
    }

    fn resolve_expr_path(
        &self,
        cont_id: ScopeId,
        expr_id: ExprId,
        ctx: NameContext,
    ) -> Option<PathResolution> {
        let path = self.expr_path(cont_id, expr_id)?;
        resolve_path(self.db, cont_id, &path, ctx)
    }

    fn expr_path(&self, cont_id: ScopeId, expr_id: ExprId) -> Option<Vec<Ident>> {
        match self.expr_in_container(cont_id, expr_id)? {
            Expr::Ident(ident) => Some(vec![ident]),
            Expr::Field { receiver, field } => {
                let mut path = self.expr_path(cont_id, receiver)?;
                path.push(field?);
                Some(path)
            }
            Expr::ElementSelect { receiver, .. } => self.expr_path(cont_id, receiver),
            _ => None,
        }
    }

    fn expr_in_container(&self, cont_id: ScopeId, expr_id: ExprId) -> Option<Expr> {
        match cont_id {
            ScopeId::File(file_id) => Some(self.db.hir_file(file_id).get(expr_id).clone()),
            ScopeId::Module(module_id) => Some(self.db.module(module_id).get(expr_id).clone()),
            ScopeId::Block(block_id) => Some(self.db.block(block_id).get(expr_id).clone()),
            ScopeId::GenerateBlock(generate_block_id) => {
                Some(self.db.generate_block(generate_block_id).get(expr_id).clone())
            }
            ScopeId::Subroutine(subroutine_id) => {
                Some(self.db.subroutine(subroutine_id.as_in_container()).get(expr_id).clone())
            }
        }
    }
}
