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
        let owner = expr.cont_id.owner(self);
        let key = crate::infer::ExprQueryKey::new(self, owner, u32::from(expr.value.into_raw()));
        crate::infer::type_of_expr_query(self, key)
    }

    pub fn infer_path_resolution(&self, res: Resolution<DefId>) -> Type {
        res.unique()
            .map(|def_id| crate::infer::type_of_def_origin_query(self, def_id.primary_origin(self)))
            .unwrap_or_else(Type::unknown)
    }
}
