use la_arena::{Arena, Idx};
use syntax::{TokenKind, ast, ptr::SyntaxNodePtr};
use utils::define_enum_deriving_from;

use super::expr::{
    declarator::{DeclsRange, impl_lower_decl},
    impl_lower_expr,
    timing_control::impl_lower_event_expr,
};
use crate::{
    db::InternDb,
    define_src,
    hir_def::{
        HirData, alloc_idx_and_src,
        expr::{
            Expr, ExprSrc, LowerExpr,
            data_ty::DataTy,
            declarator::{Declarator, DeclaratorSrc, LowerDecl},
            timing_control::{DelayControl, EventExpr, EventExprSrc, LowerEventExpr},
        },
        ty::{
            DriveStrength, NetKind, Strength, lower_drive_strength, lower_net_kind, lower_strength,
        },
    },
    source_map::SourceMap,
};

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum Declaration {
        DataDecl,
        NetDecl,
        ParamDecl,
    }
}

pub type DeclarationId = Idx<Declaration>;
define_src!(DeclarationSrc(
    ast::DataDeclaration,
    ast::NetDeclaration,
    ast::ParameterDeclaration,
    ast::TypeParameterDeclaration,
    ast::LocalVariableDeclaration
));

impl DeclarationSrc {
    pub fn ptr(&self) -> SyntaxNodePtr {
        match self {
            DeclarationSrc::DataDeclaration(ptr)
            | DeclarationSrc::NetDeclaration(ptr)
            | DeclarationSrc::ParameterDeclaration(ptr)
            | DeclarationSrc::TypeParameterDeclaration(ptr)
            | DeclarationSrc::LocalVariableDeclaration(ptr) => *ptr,
        }
    }
}

impl Declaration {
    pub fn decls(&self) -> DeclsRange {
        match self {
            Declaration::DataDecl(data_decl) => data_decl.decls.clone(),
            Declaration::NetDecl(net_decl) => net_decl.decls.clone(),
            Declaration::ParamDecl(param_decl) => param_decl.decls.clone(),
        }
    }

    pub fn ty(&self) -> DataTy {
        match self {
            Declaration::DataDecl(data_decl) => data_decl.ty,
            Declaration::NetDecl(net_decl) => net_decl.ty,
            Declaration::ParamDecl(param_decl) => param_decl.ty,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataDecl {
    pub ty: DataTy,
    pub const_kw: bool,
    pub var_kw: bool,
    pub decls: DeclsRange,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetDecl {
    pub ty: DataTy,
    pub net_kind: Option<NetKind>,
    pub delay: Option<DelayControl>,
    pub strength: Option<NetStrength>,
    pub decls: DeclsRange,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum NetStrength {
    Pull(Strength),
    Drive(DriveStrength),
    Charge(Strength),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ParamDecl {
    pub ty: DataTy,
    pub decls: DeclsRange,
}

pub(crate) struct LowerDeclarationCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) declarations: &'a mut Arena<Declaration>,
    pub(crate) declaration_srcs: &'a mut SourceMap<DeclarationSrc, Declaration>,

    pub(crate) decls: &'a mut Arena<Declarator>,
    pub(crate) decl_srcs: &'a mut SourceMap<DeclaratorSrc, Declarator>,

    pub(crate) event_exprs: &'a mut Arena<EventExpr>,
    pub(crate) event_expr_srcs: &'a mut SourceMap<EventExprSrc, EventExpr>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,
}

pub(crate) trait LowerDeclaration: LowerDecl + LowerEventExpr {
    fn declaration_ctx(&mut self) -> LowerDeclarationCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_declaration($ctx:ty, $data:ident, $src_map:ident) {
    impl $crate::hir_def::declaration::LowerDeclaration for $ctx {
        fn declaration_ctx(&mut self) -> $crate::hir_def::declaration::LowerDeclarationCtx<'_> {
            $crate::hir_def::declaration::LowerDeclarationCtx {
                db: self.db,
                declarations: &mut self.$data.declarations,
                declaration_srcs: &mut self.$src_map.declaration_srcs,
                decls: &mut self.$data.decls,
                decl_srcs: &mut self.$src_map.decl_srcs,
                event_exprs: &mut self.$data.event_exprs,
                event_expr_srcs: &mut self.$src_map.event_expr_srcs,
                exprs: &mut self.$data.exprs,
                expr_srcs: &mut self.$src_map.expr_srcs,
            }
        }
    }
}

impl_lower_expr!(LowerDeclarationCtx<'_>);
impl_lower_decl!(LowerDeclarationCtx<'_>);
impl_lower_event_expr!(LowerDeclarationCtx<'_>);

impl LowerDeclarationCtx<'_> {
    pub(crate) fn lower_data_decl(&mut self, data_decl: ast::DataDeclaration) -> DeclarationId {
        let mut const_kw = false;
        let mut var_kw = false;
        data_decl.modifiers().children().for_each(|tok| match tok.kind() {
            TokenKind::CONST_KEYWORD => const_kw = true,
            TokenKind::VAR_KEYWORD => var_kw = true,
            TokenKind::UNKNOWN => {}
            _ => unreachable!(),
        });

        let ty = self.expr_ctx().lower_data_ty(data_decl.type_());

        let parent = self.declarations.nxt_idx().into();
        let decls = self.decl_ctx().lower_declarators(data_decl.declarators(), parent);

        alloc_idx_and_src! {
            DataDecl { ty, const_kw, var_kw, decls } => self.declarations,
            data_decl => self.declaration_srcs,
        }
    }

    pub(crate) fn lower_net_decl(&mut self, net_decl: ast::NetDeclaration) -> DeclarationId {
        let net_kind = lower_net_kind(net_decl.net_type());
        let ty = self.expr_ctx().lower_data_ty(net_decl.type_());
        let delay = net_decl.delay().map(|delay| {
            use crate::hir_def::expr::timing_control::TimingControl::*;
            match self.event_expr_ctx().lower_timing_control(delay) {
                DelayControl(delay) => delay,
                _ => unreachable!(),
            }
        });

        let parent = self.declarations.nxt_idx().into();
        let decls = self.decl_ctx().lower_declarators(net_decl.declarators(), parent);

        let strength = net_decl.strength().and_then(|strength| {
            use ast::NetStrength::*;
            match strength {
                PullStrength(strength) => {
                    strength.strength().map(lower_strength).map(NetStrength::Pull)
                }
                DriveStrength(strength) => Some(NetStrength::Drive(lower_drive_strength(strength))),
                ChargeStrength(strength) => {
                    strength.strength().map(lower_strength).map(NetStrength::Charge)
                }
            }
        });

        alloc_idx_and_src! {
            NetDecl { ty, net_kind, delay, strength, decls } => self.declarations,
            net_decl => self.declaration_srcs,
        }
    }

    pub(crate) fn lower_param_decl_base(
        &mut self,
        param_decl: ast::ParameterDeclarationBase,
    ) -> DeclarationId {
        use ast::ParameterDeclarationBase::*;
        match param_decl {
            ParameterDeclaration(param_decl) => self.lower_param_decl(param_decl),
            TypeParameterDeclaration(type_param_decl) => {
                self.lower_type_param_decl(type_param_decl)
            }
        }
    }

    fn lower_type_param_decl(
        &mut self,
        type_param_decl: ast::TypeParameterDeclaration,
    ) -> DeclarationId {
        let start = self.decls.nxt_idx();
        let ty = DataTy::Builtin(
            self.db.intern_ty(crate::hir_def::expr::data_ty::BuiltinDataTy::default()),
        );
        let decls = DeclsRange::new(start..self.decls.nxt_idx());

        alloc_idx_and_src! {
            ParamDecl { ty, decls } => self.declarations,
            type_param_decl => self.declaration_srcs,
        }
    }

    fn lower_param_decl(&mut self, param_decl: ast::ParameterDeclaration) -> DeclarationId {
        let ty = self.expr_ctx().lower_data_ty(param_decl.type_());

        let parent = self.declarations.nxt_idx().into();
        let decls = self.decl_ctx().lower_declarators(param_decl.declarators(), parent);

        alloc_idx_and_src! {
            ParamDecl { ty, decls } => self.declarations,
            param_decl => self.declaration_srcs,
        }
    }
}
