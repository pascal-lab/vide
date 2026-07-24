use la_arena::{Idx, IdxRange, RawIdx};
use smallvec::SmallVec;
use syntax::{TokenKind, ast};
use utils::define_enum_deriving_from;

use super::{ExprId, data_ty::Dimension};
use crate::{
    hir_def::{
        Ident, alloc_with_source,
        declaration::DeclarationId,
        lower::{LoweringCtx, LoweringStore},
        lower_ident_opt,
        module::port::PortDeclId,
        stmt::StmtId,
    },
    source_map::{
        AstKind, FromSourceAst, IsNamedSrc, IsSrc, NamedAstId, SourceAst, ToAstNode,
        wrapped_ast_node_from_ptr,
    },
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Declarator {
    pub name: Option<Ident>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
    pub initializer: Option<ExprId>,
    pub secondary_initializer: Option<ExprId>,
    pub parent: DeclaratorParent,
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum DeclaratorParent {
        PortDeclId,
        DeclarationId, // ParamDecl, NetDecl, DataDecl
        StmtId, // similar to DataDecl
    }
}

pub type DeclId = Idx<Declarator>;

pub(crate) fn empty_decls_range() -> DeclsRange {
    let start = Idx::from_raw(RawIdx::from(0));
    DeclsRange::new(start..start)
}
pub type DeclsRange = IdxRange<Declarator>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct DeclaratorAst;

impl AstKind for DeclaratorAst {
    type Node<'a> = ast::Declarator<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct IdentifierNameAst;

impl AstKind for IdentifierNameAst {
    type Node<'a> = ast::IdentifierName<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct SpecparamDeclaratorAst;

impl AstKind for SpecparamDeclaratorAst {
    type Node<'a> = ast::SpecparamDeclarator<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum DeclaratorSrc {
    Declarator(NamedAstId<DeclaratorAst>),
    IdentifierName(NamedAstId<IdentifierNameAst>),
    SpecparamDeclarator(NamedAstId<SpecparamDeclaratorAst>),
}

impl DeclaratorSrc {
    fn node(&self) -> syntax::ptr::SyntaxNodePtr {
        match self {
            DeclaratorSrc::Declarator(src) => src.node,
            DeclaratorSrc::IdentifierName(src) => src.node,
            DeclaratorSrc::SpecparamDeclarator(src) => src.node,
        }
    }

    fn name(&self) -> Option<syntax::ptr::SyntaxTokenPtr> {
        match self {
            DeclaratorSrc::Declarator(src) => src.name,
            DeclaratorSrc::IdentifierName(src) => src.name,
            DeclaratorSrc::SpecparamDeclarator(src) => src.name,
        }
    }
}

impl IsSrc for DeclaratorSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        self.node().kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        self.node().range()
    }
}

impl IsNamedSrc for DeclaratorSrc {
    fn name_kind(&self) -> Option<TokenKind> {
        self.name().map(|name| name.kind())
    }

    fn name_range(&self) -> Option<utils::text_edit::TextRange> {
        self.name().map(|name| name.range())
    }
}

impl<'a> ToAstNode<'a, ast::Declarator<'a>> for DeclaratorSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::Declarator<'a>> {
        let DeclaratorSrc::Declarator(src) = self else { return None };
        wrapped_ast_node_from_ptr(src.node, tree)
    }
}

impl<'a> ToAstNode<'a, ast::IdentifierName<'a>> for DeclaratorSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::IdentifierName<'a>> {
        let DeclaratorSrc::IdentifierName(src) = self else { return None };
        wrapped_ast_node_from_ptr(src.node, tree)
    }
}

impl<'a> ToAstNode<'a, ast::SpecparamDeclarator<'a>> for DeclaratorSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::SpecparamDeclarator<'a>> {
        let DeclaratorSrc::SpecparamDeclarator(src) = self else { return None };
        wrapped_ast_node_from_ptr(src.node, tree)
    }
}

impl<'a> FromSourceAst<'a, ast::Declarator<'a>> for DeclaratorSrc {
    fn from_source_ast(node: SourceAst<ast::Declarator<'a>>) -> Self {
        Self::Declarator(NamedAstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::IdentifierName<'a>> for DeclaratorSrc {
    fn from_source_ast(node: SourceAst<ast::IdentifierName<'a>>) -> Self {
        Self::IdentifierName(NamedAstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::SpecparamDeclarator<'a>> for DeclaratorSrc {
    fn from_source_ast(node: SourceAst<ast::SpecparamDeclarator<'a>>) -> Self {
        Self::SpecparamDeclarator(NamedAstId::from_source_ast(node))
    }
}

impl<Store: LoweringStore> LoweringCtx<'_, Store> {
    pub(crate) fn lower_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::Declarator<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        decls_range(declarators.children().map(|decl| self.lower_declarator(decl, parent)))
    }

    pub(crate) fn lower_declarator(
        &mut self,
        declarator: ast::Declarator,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(declarator.name());
        let dimensions =
            declarator.dimensions().children().map(|dim| self.lower_dimension(dim)).collect();
        let initializer = declarator.initializer().map(|init| self.lower_expr(init.expr()));
        let data =
            Declarator { name, dimensions, initializer, secondary_initializer: None, parent };
        let file_id = self.file_id;
        let (declarators, sources) = self.declarators();
        alloc_with_source(file_id, declarators, sources, data, declarator)
    }

    pub(crate) fn lower_identifier_names<'a>(
        &mut self,
        identifiers: ast::SeparatedList<'a, ast::IdentifierName<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        decls_range(identifiers.children().map(|ident| self.lower_identifier_name(ident, parent)))
    }

    fn lower_identifier_name(
        &mut self,
        ident: ast::IdentifierName,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(ident.identifier());
        let data = Declarator {
            name,
            dimensions: SmallVec::new(),
            initializer: None,
            secondary_initializer: None,
            parent,
        };
        let file_id = self.file_id;
        let (declarators, sources) = self.declarators();
        alloc_with_source(file_id, declarators, sources, data, ident)
    }

    pub(crate) fn lower_specparam_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::SpecparamDeclarator<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        decls_range(
            declarators.children().map(|decl| self.lower_specparam_declarator(decl, parent)),
        )
    }

    fn lower_specparam_declarator(
        &mut self,
        declarator: ast::SpecparamDeclarator,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(declarator.name());
        let initializer = Some(self.lower_expr(declarator.value_1()));
        let secondary_initializer = declarator.value_2().map(|expr| self.lower_expr(expr));
        let data = Declarator {
            name,
            dimensions: SmallVec::new(),
            initializer,
            secondary_initializer,
            parent,
        };
        let file_id = self.file_id;
        let (declarators, sources) = self.declarators();
        alloc_with_source(file_id, declarators, sources, data, declarator)
    }
}

fn decls_range(mut ids: impl Iterator<Item = DeclId>) -> DeclsRange {
    let Some(first) = ids.next() else {
        return empty_decls_range();
    };
    let last = ids.last().unwrap_or(first);
    DeclsRange::new_inclusive(first..=last)
}
