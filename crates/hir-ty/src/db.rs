use std::ops::Deref;

use hir_def::{
    container::InContainer, db::HirDefDb, def_id::DefId, expr::ExprId, symbol::Resolution,
};

use crate::Type;

#[salsa::db]
pub trait TyDb: HirDefDb {}

// See `HirDefDb` for why composed Salsa database objects use `Deref`.
impl Deref for dyn TyDb {
    type Target = dyn HirDefDb;

    fn deref(&self) -> &Self::Target {
        self
    }
}

impl dyn TyDb + '_ {
    pub fn infer_expr(&self, expr: InContainer<ExprId>) -> Type {
        crate::infer::type_of_expr_query(self, expr, ())
    }

    pub fn infer_path_resolution(&self, res: Resolution<DefId>) -> Type {
        crate::infer::type_of_path_resolution_query(self, res, ())
    }
}
