use hir_def::{
    container::InContainer, db::HirDefDb, def_id::DefId, expr::ExprId, symbol::Resolution,
};

use crate::Type;

#[salsa::query_group(TyDbStorage)]
pub trait TyDb: HirDefDb {
    #[salsa::invoke(crate::infer::type_of_expr_query)]
    fn infer_expr(&self, expr: InContainer<ExprId>) -> Type;

    #[salsa::invoke(crate::infer::type_of_path_resolution_query)]
    fn infer_path_resolution(&self, res: Resolution<DefId>) -> Type;
}
