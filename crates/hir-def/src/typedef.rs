use la_arena::Idx;
use syntax::ast;

use super::{Ident, aggregate::StructId, expr::data_ty::DataTy};
use crate::{
    ast_id_map::SourceAstId,
    container::{ArenaOwnerId, InContainer},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Typedef {
    pub name: Option<Ident>,
    pub ty: Option<DataTy>,
}

pub type TypedefId = Idx<Typedef>;

pub type TypedefSrc = SourceAstId;

pub(crate) fn lower_typedef_data_ty<Ctx>(
    ctx: &mut Ctx,
    data_ty: ast::DataType,
    container_id: ArenaOwnerId,
    mut lower_struct_type: impl FnMut(&mut Ctx, ast::StructUnionType) -> StructId,
    mut lower_data_ty: impl FnMut(&mut Ctx, ast::DataType) -> DataTy,
) -> DataTy {
    match data_ty {
        ast::DataType::StructUnionType(struct_ty) => {
            let struct_id = lower_struct_type(ctx, struct_ty);
            DataTy::Struct(InContainer::new(container_id, struct_id))
        }
        other => lower_data_ty(ctx, other),
    }
}
