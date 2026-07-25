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

use crate::Type;

#[salsa::query_group(TyDbStorage)]
pub trait TyDb: HirDefDb {
    #[salsa::invoke(crate::type_infer::type_of_decl_query)]
    fn infer_decl(&self, decl: InContainer<DeclId>) -> Arc<Type>;

    #[salsa::invoke(crate::type_infer::type_of_typedef_query)]
    fn infer_typedef(&self, typedef: InContainer<TypedefId>) -> Arc<Type>;

    #[salsa::invoke(crate::type_infer::type_of_expr_query)]
    fn infer_expr(&self, expr: InContainer<ExprId>) -> Arc<Type>;

    #[salsa::invoke(crate::type_infer::type_of_path_resolution_query)]
    fn infer_path_resolution(&self, res: Resolution<DefId>) -> Arc<Type>;

    #[salsa::invoke(crate::type_infer::type_of_subroutine_port_query)]
    fn infer_subroutine_port(&self, port: InSubroutine<SubroutinePortId>) -> Arc<Type>;
}
