use hir_def::{
    container::{InContainer, InSubroutine},
    db::HirDefDb,
    def_id::DefId,
    expr::{ExprId, declarator::DeclId},
    subroutine::SubroutinePortId,
    symbol::Resolution,
    typedef::TypedefId,
};
use triomphe::Arc;

use crate::type_infer::TyResult;

#[salsa::query_group(TyDbStorage)]
pub trait TyDb: HirDefDb {
    #[salsa::invoke(crate::type_infer::type_of_decl_query)]
    fn type_of_decl(&self, decl: InContainer<DeclId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_typedef_query)]
    fn type_of_typedef(&self, typedef: InContainer<TypedefId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_expr_query)]
    fn type_of_expr(&self, expr: InContainer<ExprId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_path_resolution_query)]
    fn type_of_path_resolution(&self, res: Resolution<DefId>) -> Arc<TyResult>;

    #[salsa::invoke(crate::type_infer::type_of_subroutine_port_query)]
    fn type_of_subroutine_port(&self, port: InSubroutine<SubroutinePortId>) -> Arc<TyResult>;
}
