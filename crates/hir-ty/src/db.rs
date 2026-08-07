use std::ops::Deref;

use hir_def::{container::OwnerRef, db::HirDefDb, def_id::DefId, expr::ExprId, symbol::Resolution};

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
    pub fn infer_expr(&self, expr: OwnerRef<ExprId>) -> Type {
        let key =
            crate::infer::ExprQueryKey::new(self, expr.cont_id, u32::from(expr.value.into_raw()));
        crate::infer::type_of_expr_query(self, key)
    }

    pub fn infer_path_resolution(&self, res: Resolution<DefId>) -> Type {
        res.unique()
            .map(|def_id| crate::infer::type_of_def_origin_query(self, def_id.primary_origin(self)))
            .unwrap_or_else(Type::unknown)
    }
}
