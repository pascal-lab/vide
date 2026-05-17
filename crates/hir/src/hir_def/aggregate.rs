use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode, DataType, StructUnionType},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::AstNodeExt,
};
use utils::text_edit::TextRange;

use super::{Ident, expr::data_ty::DataTy, lower_ident_opt};
use crate::{
    container::{ContainerId, InContainer},
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
};

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
    container_id: ContainerId,
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
            let ty = InContainer::new(container_id, member_ty);
            members.push(StructMember { name, ty: Some(ty) });
        }
    }

    StructDef { kind, name: None, packed, signing, tagged, members }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StructSrc {
    pub node: SyntaxNodePtr,
}

impl IsSrc for StructSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl<'a> ToAstNode<'a, ast::StructUnionType<'a>> for StructSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::StructUnionType<'a>> {
        let mut node = self.node.to_node(tree)?;
        while !ast::StructUnionType::can_cast(node.kind()) {
            node = node.children().find_map(|elem| elem.as_node())?;
        }
        ast::StructUnionType::cast(node)
    }
}

impl From<ast::StructUnionType<'_>> for StructSrc {
    fn from(node: ast::StructUnionType<'_>) -> Self {
        StructSrc { node: AstNodeExt::to_ptr(&node) }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for ClassSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for ClassSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, ast::ClassDeclaration<'a>> for ClassSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::ClassDeclaration<'a>> {
        let mut node = self.node.to_node(tree)?;
        while !ast::ClassDeclaration::can_cast(node.kind()) {
            node = node.children().find_map(|elem| elem.as_node())?;
        }
        ast::ClassDeclaration::cast(node)
    }
}

impl From<ast::ClassDeclaration<'_>> for ClassSrc {
    fn from(node: ast::ClassDeclaration<'_>) -> Self {
        let syntax = node.syntax();
        let name = node.name().map(|name| SyntaxTokenPtr::from_token_in(syntax, name));
        ClassSrc { node: AstNodeExt::to_ptr(&node), name }
    }
}
