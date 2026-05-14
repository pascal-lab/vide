use la_arena::{Arena, Idx};
use smallvec::SmallVec;
use syntax::ast;

use crate::{
    db::InternDb,
    define_src, define_src_with_name,
    hir_def::{
        HirData, Ident, alloc_idx_and_src,
        expr::{Expr, ExprId, ExprSrc, LowerExpr, data_ty::Dimension, impl_lower_expr},
        lower_ident_opt,
    },
    source_map::SourceMap,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Instantiation {
    pub module_name: Option<Ident>,
    pub param_assigns: SmallVec<[ParamAssignId; 1]>,
    pub instances: SmallVec<[InstanceId; 1]>,
}

pub type InstantiationId = Idx<Instantiation>;

define_src!(InstantiationSrc(ast::HierarchyInstantiation, ast::PrimitiveInstantiation));

impl From<InstantiationSrc> for syntax::ptr::SyntaxNodePtr {
    fn from(src: InstantiationSrc) -> Self {
        match src {
            InstantiationSrc::HierarchyInstantiation(ptr) => ptr,
            InstantiationSrc::PrimitiveInstantiation(ptr) => ptr,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Instance {
    pub name: Option<Ident>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
    pub connections: Vec<PortConnId>,
    pub parent: InstantiationId,
}

pub type InstanceId = Idx<Instance>;

define_src_with_name!(InstanceSrc(ast::HierarchicalInstance));

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParamAssign {
    Ordered(ExprId),
    Named(Option<Ident>, Option<ExprId>),
}

pub type ParamAssignId = Idx<ParamAssign>;

define_src_with_name!(ParamAssignSrc(ast::ParamAssignment));

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortConn {
    Empty,
    Ordered(ExprId),
    Named(Option<Ident>, Option<ExprId>), // .a(b) or .a or .(b)
    Wildcard,
}

pub type PortConnId = Idx<PortConn>;

define_src_with_name!(PortConnSrc(ast::PortConnection));

pub(crate) struct LowerInstantiationCtx<'a> {
    pub(crate) db: &'a dyn InternDb,

    pub(crate) instantiations: &'a mut Arena<Instantiation>,
    pub(crate) instantiation_srcs: &'a mut SourceMap<InstantiationSrc, Instantiation>,

    pub(crate) inst_param_assigns: &'a mut Arena<ParamAssign>,
    pub(crate) inst_param_assign_srcs: &'a mut SourceMap<ParamAssignSrc, ParamAssign>,

    pub(crate) instances: &'a mut Arena<Instance>,
    pub(crate) instance_srcs: &'a mut SourceMap<InstanceSrc, Instance>,

    pub(crate) inst_port_conns: &'a mut Arena<PortConn>,
    pub(crate) inst_port_conn_srcs: &'a mut SourceMap<PortConnSrc, PortConn>,

    pub(crate) exprs: &'a mut Arena<Expr>,
    pub(crate) expr_srcs: &'a mut SourceMap<ExprSrc, Expr>,
}

pub(crate) trait LowerInstantiation: LowerExpr {
    fn instantiation_ctx(&mut self) -> LowerInstantiationCtx<'_>;
}

pub(in crate::hir_def) macro impl_lower_instantiation($ctx:ty, $data:ident, $src_map:ident) {
    impl $crate::hir_def::module::instantiation::LowerInstantiation for $ctx {
        fn instantiation_ctx(
            &mut self,
        ) -> $crate::hir_def::module::instantiation::LowerInstantiationCtx<'_> {
            $crate::hir_def::module::instantiation::LowerInstantiationCtx {
                db: self.db,
                instantiations: &mut self.$data.instantiations,
                instantiation_srcs: &mut self.$src_map.instantiation_srcs,
                inst_param_assigns: &mut self.$data.inst_param_assigns,
                inst_param_assign_srcs: &mut self.$src_map.inst_param_assign_srcs,
                instances: &mut self.$data.instances,
                instance_srcs: &mut self.$src_map.instance_srcs,
                inst_port_conns: &mut self.$data.inst_port_conns,
                inst_port_conn_srcs: &mut self.$src_map.inst_port_conn_srcs,
                exprs: &mut self.$data.exprs,
                expr_srcs: &mut self.$src_map.expr_srcs,
            }
        }
    }
}

impl_lower_expr!(LowerInstantiationCtx<'_>);

impl LowerInstantiationCtx<'_> {
    pub(crate) fn lower_instantiation(
        &mut self,
        instance: ast::HierarchyInstantiation,
    ) -> InstantiationId {
        let module_name = lower_ident_opt(instance.type_());
        let param_assigns = self.lower_param_assign(instance.parameters());

        let next_instantiation_id = self.instantiations.nxt_idx();
        let instances = instance
            .instances()
            .children()
            .map(|inst| self.lower_instance(inst, next_instantiation_id))
            .collect();
        alloc_idx_and_src! {
            Instantiation { module_name, param_assigns, instances } => self.instantiations,
            instance => self.instantiation_srcs,
        }
    }

    pub(crate) fn lower_primitive_instantiation(
        &mut self,
        inst: ast::PrimitiveInstantiation,
    ) -> InstantiationId {
        let module_name = lower_ident_opt(inst.type_());
        let param_assigns = SmallVec::new();

        let next_instantiation_id = self.instantiations.nxt_idx();
        let instances = inst
            .instances()
            .children()
            .map(|hier| self.lower_instance(hier, next_instantiation_id))
            .collect();

        alloc_idx_and_src! {
            Instantiation { module_name, param_assigns, instances } => self.instantiations,
            inst => self.instantiation_srcs,
        }
    }

    fn lower_param_assign(
        &mut self,
        assigns: Option<ast::ParameterValueAssignment>,
    ) -> SmallVec<[ParamAssignId; 1]> {
        let Some(assigns) = assigns else {
            return SmallVec::new();
        };
        assigns
            .parameters()
            .children()
            .map(|assign| {
                use ast::ParamAssignment::*;
                let hir_assign = match assign {
                    OrderedParamAssignment(assign) => {
                        ParamAssign::Ordered(self.expr_ctx().lower_expr(assign.expr()))
                    }
                    NamedParamAssignment(assign) => {
                        let name = lower_ident_opt(assign.name());
                        let expr = assign.expr().map(|expr| self.expr_ctx().lower_expr(expr));
                        ParamAssign::Named(name, expr)
                    }
                };

                alloc_idx_and_src! {
                    hir_assign => self.inst_param_assigns,
                    assign => self.inst_param_assign_srcs,
                }
            })
            .collect()
    }

    fn lower_instance(
        &mut self,
        instance: ast::HierarchicalInstance,
        parent: InstantiationId,
    ) -> InstanceId {
        let connections = instance
            .connections()
            .children()
            .map(|conn| {
                use ast::PortConnection::*;
                let hir_conn = match conn {
                    EmptyPortConnection(_) => PortConn::Empty,
                    OrderedPortConnection(conn) => {
                        let expr = self.expr_ctx().lower_property_expr(conn.expr());
                        PortConn::Ordered(expr)
                    }
                    NamedPortConnection(conn) => {
                        let name = lower_ident_opt(conn.name());
                        let expr =
                            conn.expr().map(|expr| self.expr_ctx().lower_property_expr(expr));
                        PortConn::Named(name, expr)
                    }
                    WildcardPortConnection(_) => PortConn::Wildcard,
                };
                alloc_idx_and_src! {
                    hir_conn => self.inst_port_conns,
                    conn => self.inst_port_conn_srcs,
                }
            })
            .collect();

        let (name, dimensions) = instance
            .decl()
            .map(|decl| {
                let name = lower_ident_opt(decl.name());
                let dimensions = decl
                    .dimensions()
                    .children()
                    .map(|dim| self.expr_ctx().lower_dimension(dim))
                    .collect();
                (name, dimensions)
            })
            .unwrap_or_default();

        alloc_idx_and_src! {
            Instance { name, dimensions, connections, parent } => self.instances,
            instance => self.instance_srcs,
        }
    }
}
