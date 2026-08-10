use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, StructUnionMember, StructUnionType},
    has_name::HasName,
};

use super::{
    Ident,
    expr::{
        ExprId,
        data_ty::{DataTy, Dimension},
    },
    lower_ident_opt,
};
use crate::{container::OwnerRef, owner::OwnerId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructKind {
    Struct,
    Union,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructMember {
    pub name: Option<Ident>,
    pub ty: Option<OwnerRef<DataTy>>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
    pub initializer: Option<ExprId>,
    pub random: bool,
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
    lower_member: impl FnMut(StructUnionMember) -> SmallVec<[StructMember; 4]>,
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

    let members = struct_ty
        .members()
        .children()
        .flat_map(lower_member)
        .map(|mut member| {
            if let Some(ty) = member.ty.as_mut() {
                ty.cont_id = container_id;
            }
            member
        })
        .collect();

    StructDef { kind, name: None, packed, signing, tagged, members }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumMember {
    pub name: Option<Ident>,
    pub initializer: Option<ExprId>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDef {
    pub base_ty: Option<DataTy>,
    pub members: SmallVec<[EnumMember; 8]>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
}

pub type EnumId = Idx<EnumDef>;

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
    pub ty: Option<OwnerRef<DataTy>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassDef {
    pub name: Option<Ident>,
    pub base_class_name: Option<Ident>,
    pub members: SmallVec<[ClassMember; 4]>,
}

pub type ClassId = Idx<ClassDef>;

pub(crate) fn lower_class_def(class: ast::ClassDeclaration<'_>) -> ClassDef {
    let base_class_name = class
        .extends_clause()
        .and_then(|extends| extends.base_name().as_identifier_name())
        .and_then(|name| lower_ident_opt(name.identifier()));
    let members = class
        .items()
        .children()
        .filter_map(|member| match member {
            ast::Member::ClassPropertyDeclaration(property) => {
                let name = match property.declaration() {
                    ast::Member::DataDeclaration(declaration) => declaration
                        .declarators()
                        .children()
                        .next()
                        .and_then(|decl| lower_ident_opt(decl.name())),
                    _ => None,
                };
                Some(ClassMember { name, kind: ClassMemberKind::Property, ty: None })
            }
            ast::Member::ClassMethodDeclaration(method) => Some(ClassMember {
                name: lower_ident_opt(method.declaration().name()),
                kind: ClassMemberKind::Method,
                ty: None,
            }),
            ast::Member::ClassMethodPrototype(method) => Some(ClassMember {
                name: method
                    .prototype()
                    .name()
                    .as_identifier_name()
                    .and_then(|name| lower_ident_opt(name.identifier())),
                kind: ClassMemberKind::Method,
                ty: None,
            }),
            _ => None,
        })
        .collect();
    ClassDef { name: lower_ident_opt(class.name()), base_class_name, members }
}
