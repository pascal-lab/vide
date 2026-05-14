use la_arena::{Arena, Idx, IdxRange};
use smallvec::SmallVec;
use syntax::ast;
use utils::define_enum_deriving_from;

use super::{Expr, ExprId, ExprSrc, LowerExpr, data_ty::Dimension, impl_lower_expr};
use crate::{
    db::InternDb,
    define_src_with_name,
    hir_def::{
        HirData, Ident, alloc_idx_and_src, declaration::DeclarationId, lower_ident_opt,
        module::port::PortDeclId, stmt::StmtId,
    },
    source_map::SourceMap,
};

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Declarator {
    pub name: Option<Ident>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
    pub initializer: Option<ExprId>,
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

define_src_with_name!(DeclaratorSrc(ast::Declarator, ast::IdentifierName));

pub(crate) struct LowerDeclCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
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
            Declarator { name, dimensions, initializer, parent } => self.decls,
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
            Declarator { name, dimensions: SmallVec::new(), initializer: None, parent } => self.decls,
            ident => self.decl_srcs,
        }
    }
}
