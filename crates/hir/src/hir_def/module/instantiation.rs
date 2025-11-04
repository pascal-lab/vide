use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast;

use super::LowerModuleCtx;
use crate::{
    define_src, define_src_with_name,
    hir_def::{
        HirData, Ident, alloc_idx_and_src,
        expr::{ExprId, LowerExpr, data_ty::Dimension},
        lower_ident_opt,
    },
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

impl LowerModuleCtx<'_> {
    pub(crate) fn lower_instantiation(
        &mut self,
        instance: ast::HierarchyInstantiation,
    ) -> InstantiationId {
        let module_name = lower_ident_opt(instance.type_());
        let param_assigns = self.lower_param_assign(instance.parameters());

        let next_instantiation_id = self.module.instantiations.nxt_idx();
        let instances = instance
            .instances()
            .children()
            .map(|inst| self.lower_instance(inst, next_instantiation_id))
            .collect();
        alloc_idx_and_src! {
            Instantiation { module_name, param_assigns, instances } => self.module.instantiations,
            instance => self.module_source_map.instantiation_srcs,
        }
    }

    pub(crate) fn lower_primitive_instantiation(
        &mut self,
        inst: ast::PrimitiveInstantiation,
    ) -> InstantiationId {
        let module_name = lower_ident_opt(inst.type_());
        let param_assigns = SmallVec::new();

        let next_instantiation_id = self.module.instantiations.nxt_idx();
        let instances = inst
            .instances()
            .children()
            .map(|hier| self.lower_instance(hier, next_instantiation_id))
            .collect();

        alloc_idx_and_src! {
            Instantiation { module_name, param_assigns, instances } => self.module.instantiations,
            inst => self.module_source_map.instantiation_srcs,
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
                    hir_assign => self.module.inst_param_assigns,
                    assign => self.module_source_map.inst_param_assign_srcs,
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
                    hir_conn => self.module.inst_port_conns,
                    conn => self.module_source_map.inst_port_conn_srcs,
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
            Instance { name, dimensions, connections, parent } => self.module.instances,
            instance => self.module_source_map.instance_srcs,
        }
    }
}
