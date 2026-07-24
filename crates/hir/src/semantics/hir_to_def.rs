use rustc_hash::FxHashMap;
use utils::get::GetRef;

use super::Source2DefCtx;
use crate::{
    container::{InContainer, ScopeId},
    def_id::DefId,
    hir_def::{
        Ident,
        expr::{Expr, ExprId},
    },
    semantics::pathres::{descend_scope, name_scope, resolve_name, resolve_path},
    symbol::{NameContext, Resolution},
};

#[derive(Default, Debug)]
pub(super) struct Hir2DefCache {
    expr_map: FxHashMap<InContainer<ExprId>, Resolution<DefId>>,
    name_map: FxHashMap<InContainer<Ident>, Resolution<DefId>>,
}

impl Source2DefCtx<'_, '_> {
    pub(super) fn expr_to_def(
        &mut self,
        InContainer { cont_id, value: expr_id }: InContainer<ExprId>,
    ) -> Option<Resolution<DefId>> {
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
            ScopeId::ClockingBlock(_) | ScopeId::Checker(_) | ScopeId::Covergroup(_) => None,
        }
    }

    pub(super) fn name_to_def(
        &mut self,
        InContainer { cont_id, value: ident }: InContainer<Ident>,
        name_ctx: NameContext,
    ) -> Option<Resolution<DefId>> {
        let res = resolve_name(self.db, cont_id, &ident, name_ctx).into_option()?;
        self.hir_cache.name_map.insert(InContainer::new(cont_id, ident), res.clone());
        Some(res)
    }

    fn resolve_member_from_resolution(
        &mut self,
        res: Resolution<DefId>,
        field: &Ident,
    ) -> Option<Resolution<DefId>> {
        let mut defs = smallvec::SmallVec::<[DefId; 3]>::new();
        for def_id in res.candidates() {
            let Some(scope_id) = descend_scope(self.db, *def_id) else {
                continue;
            };
            for child in
                name_scope(self.db, scope_id).lookup(NameContext::Value, field).into_candidates()
            {
                if !defs.contains(&child) {
                    defs.push(child);
                }
            }
        }
        Resolution::from_candidates(defs).into_option()
    }

    fn resolve_expr_path(
        &self,
        cont_id: ScopeId,
        expr_id: ExprId,
        ctx: NameContext,
    ) -> Option<Resolution<DefId>> {
        let path = self.expr_path(cont_id, expr_id)?;
        resolve_path(self.db, cont_id, &path, ctx).into_option()
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
            ScopeId::ClockingBlock(_) | ScopeId::Checker(_) | ScopeId::Covergroup(_) => None,
        }
    }
}
