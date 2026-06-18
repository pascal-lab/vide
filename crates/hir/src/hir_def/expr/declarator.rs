use la_arena::{Arena, Idx, IdxRange};
use smallvec::SmallVec;
use syntax::{TokenKind, ast};
use utils::define_enum_deriving_from;

use super::{Expr, ExprId, ExprSrc, LowerExpr, data_ty::Dimension, impl_lower_expr};
use crate::{
    db::InternDb,
    file::HirFileId,
    hir_def::{
        HirData, Ident, alloc_idx_and_src, declaration::DeclarationId, lower_ident_opt,
        module::port::PortDeclId, stmt::StmtId,
    },
    source_map::{
        AstKind, FromSourceAst, IsNamedSrc, IsSrc, NamedAstId, SourceAst, SourceMap, ToAstNode,
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

pub(crate) struct LowerDeclCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) decls: &'a mut Arena<Declarator>,
    pub(crate) decl_srcs: &'a mut SourceMap<DeclaratorSrc, Declarator>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,
}

pub(crate) trait LowerDecl: LowerExpr {
    fn decl_ctx(&mut self) -> LowerDeclCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_decl {
    ($ctx:ty $(,$data:ident, $src_map:ident)?) => {
        impl $crate::hir_def::expr::declarator::LowerDecl for $ctx {
            fn decl_ctx(&mut self) -> $crate::hir_def::expr::declarator::LowerDeclCtx<'_> {
                $crate::hir_def::expr::declarator::LowerDeclCtx {
                    db: self.db,
                    file_id: self.file_id,
                    decls: &mut self.$($data.)?decls,
                    decl_srcs: &mut self.$($src_map.)?decl_srcs,
                    exprs: &mut self.$($data.)?exprs,
                    expr_srcs: &mut self.$($src_map.)?expr_srcs,
                }
            }
        }
    },
}

impl_lower_expr!(LowerDeclCtx<'_>);

impl LowerDeclCtx<'_> {
    pub(crate) fn lower_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::Declarator<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        let start = self.decls.nxt_idx();
        declarators.children().for_each(|decl| {
            self.lower_declarator(decl, parent);
        });
        let end = self.decls.nxt_idx();
        DeclsRange::new(start..end)
    }

    pub(crate) fn lower_declarator(
        &mut self,
        declarator: ast::Declarator,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(declarator.name());
        let dimensions = declarator
            .dimensions()
            .children()
            .map(|dim| self.expr_ctx().lower_dimension(dim))
            .collect();
        let initializer =
            declarator.initializer().map(|init| self.expr_ctx().lower_expr(init.expr()));
        alloc_idx_and_src! {
            self.file_id;
            Declarator {
                name,
                dimensions,
                initializer,
                secondary_initializer: None,
                parent
            } => self.decls,
            declarator => self.decl_srcs,
        }
    }

    pub(crate) fn lower_identifier_names<'a>(
        &mut self,
        identifiers: ast::SeparatedList<'a, ast::IdentifierName<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        let start = self.decls.nxt_idx();
        identifiers.children().for_each(|ident| {
            self.lower_identifier_name(ident, parent);
        });
        let end = self.decls.nxt_idx();
        DeclsRange::new(start..end)
    }

    fn lower_identifier_name(
        &mut self,
        ident: ast::IdentifierName,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(ident.identifier());
        alloc_idx_and_src! {
            self.file_id;
            Declarator {
                name,
                dimensions: SmallVec::new(),
                initializer: None,
                secondary_initializer: None,
                parent
            } => self.decls,
            ident => self.decl_srcs,
        }
    }

    pub(crate) fn lower_specparam_declarators<'a>(
        &mut self,
        declarators: ast::SeparatedList<'a, ast::SpecparamDeclarator<'a>>,
        parent: DeclaratorParent,
    ) -> DeclsRange {
        let start = self.decls.nxt_idx();
        declarators.children().for_each(|decl| {
            self.lower_specparam_declarator(decl, parent);
        });
        let end = self.decls.nxt_idx();
        DeclsRange::new(start..end)
    }

    fn lower_specparam_declarator(
        &mut self,
        declarator: ast::SpecparamDeclarator,
        parent: DeclaratorParent,
    ) -> DeclId {
        let name = lower_ident_opt(declarator.name());
        let initializer = Some(self.expr_ctx().lower_expr(declarator.value_1()));
        let secondary_initializer =
            declarator.value_2().map(|expr| self.expr_ctx().lower_expr(expr));
        alloc_idx_and_src! {
            self.file_id;
            Declarator {
                name,
                dimensions: SmallVec::new(),
                initializer,
                secondary_initializer,
                parent
            } => self.decls,
            declarator => self.decl_srcs,
        }
    }
}
