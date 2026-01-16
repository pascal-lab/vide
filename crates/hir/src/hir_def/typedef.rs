use la_arena::Idx;
use syntax::{
    SyntaxKind, TokenKind,
    ast::{self, AstNode},
    ptr::{SyntaxNodePtr, SyntaxTokenPtr},
    slang_ext::AstNodeExt,
};
use utils::text_edit::TextRange;

use super::{Ident, aggregate::StructId, expr::data_ty::DataTy};
use crate::{
    container::{ContainerId, InContainer},
    source_map::{IsNamedSrc, IsSrc, ToAstNode},
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Typedef {
    pub name: Option<Ident>,
    pub ty: Option<DataTy>,
}

pub type TypedefId = Idx<Typedef>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct TypedefSrc {
    pub node: SyntaxNodePtr,
    pub name: Option<SyntaxTokenPtr>,
}

impl IsSrc for TypedefSrc {
    #[inline]
    fn kind(&self) -> SyntaxKind {
        self.node.kind()
    }

    #[inline]
    fn range(&self) -> TextRange {
        self.node.range()
    }
}

impl IsNamedSrc for TypedefSrc {
    #[inline]
    fn name_kind(&self) -> Option<TokenKind> {
        self.name.map(|name| name.kind())
    }

    #[inline]
    fn name_range(&self) -> Option<TextRange> {
        self.name.map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, ast::TypedefDeclaration<'a>> for TypedefSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::TypedefDeclaration<'a>> {
        let mut node = self.node.to_node(tree)?;
        while !ast::TypedefDeclaration::can_cast(node.kind()) {
            node = node.children().find_map(|elem| elem.as_node()).unwrap();
        }
        ast::TypedefDeclaration::cast(node)
    }
}

impl From<ast::TypedefDeclaration<'_>> for TypedefSrc {
    fn from(node: ast::TypedefDeclaration<'_>) -> Self {
        let name_token = node.name();
        TypedefSrc {
            node: AstNodeExt::to_ptr(&node),
            name: name_token.map(SyntaxTokenPtr::from_token),
        }
    }
}

impl TypedefSrc {
    pub fn ptr(&self) -> SyntaxNodePtr {
        self.node
    }
}

pub(crate) fn lower_typedef_data_ty<Ctx>(
    ctx: &mut Ctx,
    data_ty: ast::DataType,
    container_id: ContainerId,
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
