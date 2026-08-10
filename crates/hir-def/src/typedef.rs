use la_arena::Idx;
use syntax::{TokenKind, ast};

use super::{Ident, aggregate::StructId, expr::data_ty::DataTy};
use crate::{container::OwnerRef, owner::OwnerId};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Typedef {
    pub name: Option<Ident>,
    pub ty: Option<DataTy>,
    pub forward_kind: Option<ForwardTypedefKind>,
}

pub type TypedefId = Idx<Typedef>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ForwardTypedefKind {
    Unspecified,
    Enum,
    Struct,
    Union,
    Class,
    InterfaceClass,
}

impl ForwardTypedefKind {
    pub(crate) fn from_restriction(restriction: ast::ForwardTypeRestriction) -> Option<Self> {
        match (
            restriction.keyword_1().map(|token| token.kind()),
            restriction.keyword_2().map(|token| token.kind()),
        ) {
            (Some(TokenKind::ENUM_KEYWORD), None) => Some(Self::Enum),
            (Some(TokenKind::STRUCT_KEYWORD), None) => Some(Self::Struct),
            (Some(TokenKind::UNION_KEYWORD), None) => Some(Self::Union),
            (Some(TokenKind::CLASS_KEYWORD), None) => Some(Self::Class),
            (Some(TokenKind::INTERFACE_KEYWORD), Some(TokenKind::CLASS_KEYWORD)) => {
                Some(Self::InterfaceClass)
            }
            _ => None,
        }
    }
}

pub(crate) fn lower_typedef_data_ty<Ctx>(
    ctx: &mut Ctx,
    data_ty: ast::DataType,
    container_id: OwnerId,
    mut lower_struct_type: impl FnMut(&mut Ctx, ast::StructUnionType) -> StructId,
    mut lower_data_ty: impl FnMut(&mut Ctx, ast::DataType) -> DataTy,
) -> DataTy {
    match data_ty {
        ast::DataType::StructUnionType(struct_ty) => {
            let struct_id = lower_struct_type(ctx, struct_ty);
            DataTy::Struct(OwnerRef::new(container_id, struct_id))
        }
        other => lower_data_ty(ctx, other),
    }
}
