use la_arena::{Arena, Idx, IdxRange};
use smallvec::SmallVec;
use syntax::ast;
use utils::define_enum_deriving_from;

use super::{Expr, ExprId, ExprSrc, LowerExpr, data_ty::Dimension};
use crate::{
    alloc_idx_and_src,
    db::InternDb,
    define_src,
    hir_def::{
        Ident,
        declaration::DeclarationId,
        lower_ident_opt,
        module::port::{AnsiPortId, ParamPortId, PortDeclId},
        stmt::StmtId,
    },
    impl_lower_expr,
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
        ParamPortId,
        AnsiPortId,
        PortDeclId,
        DeclarationId,
        StmtId,
    }
}

pub type DeclId = Idx<Declarator>;
pub type DeclIdRange = IdxRange<Declarator>;

define_src!(DeclaratorSrc(ast::Declarator));

pub(crate) struct LowerDeclCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) decls: &'a mut Arena<Declarator>,
    pub(crate) decl_srcs: &'a mut SourceMap<DeclaratorSrc, Declarator>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,
}

pub(crate) trait LowerDecl: LowerExpr {
    fn decl_ctx(&mut self) -> LowerDeclCtx;
}

#[macro_export]
macro_rules! impl_lower_decl {
    ($ctx:ty $(,$data:ident, $src_map:ident)?) => {
        impl $crate::hir_def::expr::declarator::LowerDecl for $ctx {
            fn decl_ctx(&mut self) -> $crate::hir_def::expr::declarator::LowerDeclCtx {
                $crate::hir_def::expr::declarator::LowerDeclCtx {
                    db: self.db,
                    decls: &mut self.$($data.)?decls,
                    decl_srcs: &mut self.$($src_map.)?decl_srcs,
                    exprs: &mut self.$($data.)?exprs,
                    expr_srcs: &mut self.$($src_map.)?expr_srcs,
                }
            }
        }
    };
}

impl_lower_expr!(LowerDeclCtx<'_>);

impl LowerDeclCtx<'_> {
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
}
