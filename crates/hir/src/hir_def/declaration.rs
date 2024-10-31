use la_arena::{Arena, Idx, IdxRange};
use syntax::{TokenKind, ast};
use utils::define_enum_deriving_from;

use crate::{
    alloc_idx_and_src,
    db::InternDb,
    define_src,
    hir_def::{
        arena_nxt_idx,
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
    impl_lower_decl, impl_lower_event_expr, impl_lower_expr,
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
define_src!(DeclarationSrc(ast::DataDeclaration, ast::NetDeclaration, ast::ParameterDeclaration));

impl Declaration {
    pub fn decls(&self) -> IdxRange<Declarator> {
        match self {
            Declaration::DataDecl(data_decl) => data_decl.decls.clone(),
            Declaration::NetDecl(net_decl) => net_decl.decls.clone(),
            Declaration::ParamDecl(param_decl) => param_decl.decls.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DataDecl {
    pub ty: DataTy,
    pub const_kw: bool,
    pub var_kw: bool,
    pub decls: IdxRange<Declarator>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NetDecl {
    pub ty: DataTy,
    pub net_kind: Option<NetKind>,
    pub delay: Option<DelayControl>,
    pub strength: Option<NetStrength>,
    pub decls: IdxRange<Declarator>,
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
    pub decls: IdxRange<Declarator>,
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

#[macro_export]
macro_rules! impl_lower_declaration {
    ($ctx:ty, $data:ident, $src_map:ident) => {
        impl $crate::hir_def::declaration::LowerDeclaration for $ctx {
            fn declaration_ctx(&mut self) -> $crate::hir_def::declaration::LowerDeclarationCtx {
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
    };
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

        let decls = {
            let next_declaration_idx = arena_nxt_idx(self.declarations).into();
            let start = arena_nxt_idx(self.decls);
            for decl in data_decl.declarators().children() {
                self.decl_ctx().lower_declarator(decl, next_declaration_idx);
            }
            let end = arena_nxt_idx(self.decls);
            IdxRange::new(start..end)
        };

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

        let decls = {
            let next_declaration_idx = arena_nxt_idx(self.declarations).into();
            let start = arena_nxt_idx(self.decls);
            for decl in net_decl.declarators().children() {
                self.decl_ctx().lower_declarator(decl, next_declaration_idx);
            }
            let end = arena_nxt_idx(self.decls);
            IdxRange::new(start..end)
        };

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

    pub(crate) fn lower_param_decl_stmt(
        &mut self,
        param_decl: ast::ParameterDeclarationStatement,
    ) -> DeclarationId {
        use ast::ParameterDeclarationBase::*;
        match param_decl.parameter() {
            ParameterDeclaration(param_decl) => self.lower_param_decl(param_decl),
            TypeParameterDeclaration(_) => unimplemented!(),
        }
    }

    fn lower_param_decl(&mut self, param_decl: ast::ParameterDeclaration) -> DeclarationId {
        let ty = self.expr_ctx().lower_data_ty(param_decl.type_());
        let decls = {
            let next_declaration_idx = arena_nxt_idx(self.declarations).into();
            let start = arena_nxt_idx(self.decls);
            for decl in param_decl.declarators().children() {
                self.decl_ctx().lower_declarator(decl, next_declaration_idx);
            }
            let end = arena_nxt_idx(self.decls);
            IdxRange::new(start..end)
        };

        alloc_idx_and_src! {
            ParamDecl { ty, decls } => self.declarations,
            param_decl => self.declaration_srcs,
        }
    }
}
