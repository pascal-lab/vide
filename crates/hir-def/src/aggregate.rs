use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{DataType, StructUnionType},
};

use super::{Ident, expr::data_ty::DataTy, lower_ident_opt};
use crate::{container::InContainer, owner::OwnerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructKind {
    Struct,
    Union,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMember {
    pub name: Option<Ident>,
    pub ty: Option<InContainer<DataTy>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    pub kind: StructKind,
    pub name: Option<Ident>,
    pub packed: bool,
    pub signing: Option<bool>,
    pub tagged: bool,
    pub members: SmallVec<[StructMember; 4]>,
}

pub type StructId = Idx<StructDef>;

pub(crate) fn lower_struct_def(
    struct_ty: StructUnionType,
    container_id: OwnerId,
    mut lower_data_ty: impl FnMut(DataType) -> DataTy,
) -> StructDef {
    let kind = match struct_ty {
        StructUnionType::StructType(_) => StructKind::Struct,
        StructUnionType::UnionType(_) => StructKind::Union,
    };

    let packed = struct_ty.packed().is_some();
    let tagged = struct_ty
        .tagged_or_soft()
        .map(|tok| tok.kind() == TokenKind::TAGGED_KEYWORD)
        .unwrap_or(false);
    let signing = struct_ty.signing().and_then(|tok| match tok.kind() {
        TokenKind::SIGNED_KEYWORD => Some(true),
        TokenKind::UNSIGNED_KEYWORD => Some(false),
        _ => None,
    });

    let mut members = SmallVec::<[StructMember; 4]>::new();
    for member in struct_ty.members().children() {
        let member_ty = lower_data_ty(member.type_());
        for declarator in member.declarators().children() {
            let name = lower_ident_opt(declarator.name());
            let ty = InContainer::new(container_id, member_ty.clone());
            members.push(StructMember { name, ty: Some(ty) });
        }
    }

    StructDef { kind, name: None, packed, signing, tagged, members }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClassMemberKind {
    Property,
    Method,
    Typedef,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassMember {
    pub name: Option<Ident>,
    pub kind: ClassMemberKind,
    pub ty: Option<InContainer<DataTy>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: Option<Ident>,
    pub base_class_name: Option<Ident>,
    pub members: SmallVec<[ClassMember; 4]>,
}

pub type ClassId = Idx<ClassDef>;
