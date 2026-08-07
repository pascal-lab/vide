use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{
    SyntaxToken,
    ast::{self, AstNode},
};

use crate::{
    Ident,
    expr::{ExprId, data_ty::Dimension},
    lower::{LoweringCtx, ModuleItemStore},
    lower_ident_opt,
};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Instantiation {
    pub module_name: Option<Ident>,
    pub param_assigns: SmallVec<[ParamAssignId; 1]>,
    pub instances: SmallVec<[InstanceId; 1]>,
}

pub type InstantiationId = Idx<Instantiation>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Instance {
    pub name: Option<Ident>,
    pub dimensions: SmallVec<[Option<Dimension>; 2]>,
    pub connections: Vec<PortConnId>,
    pub parent: InstantiationId,
}

pub type InstanceId = Idx<Instance>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParamAssign {
    Ordered(ExprId),
    Named(Option<Ident>, Option<ExprId>),
}

pub type ParamAssignId = Idx<ParamAssign>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortConn {
    Empty,
    Ordered(ExprId),
    Named(Option<Ident>, Option<ExprId>), // .a(b) or .a or .(b)
    Wildcard,
}

pub type PortConnId = Idx<PortConn>;

impl<Store: ModuleItemStore> LoweringCtx<Store> {
    fn reserve_instantiation<'ast, Ast>(&mut self, ast: Ast) -> InstantiationId
    where
        Ast: syntax::ast::AstNode<'ast>,
    {
        let source = self.source_id(ast.syntax());
        let (instantiations, sources) = self.instantiations();
        crate::alloc_with_source_entry(instantiations, sources, Instantiation::default(), source)
    }

    fn finish_instantiation(&mut self, id: InstantiationId, instantiation: Instantiation) {
        self.instantiations().0[id] = instantiation;
    }

    pub(crate) fn lower_instantiation(
        &mut self,
        instance: ast::HierarchyInstantiation,
    ) -> InstantiationId {
        let parent = self.reserve_instantiation(instance);
        let module_name = lower_ident_opt(instance.type_());
        let param_assigns = self.lower_param_assign(instance.parameters());
        let instances =
            instance.instances().children().map(|inst| self.lower_instance(inst, parent)).collect();
        self.finish_instantiation(parent, Instantiation { module_name, param_assigns, instances });
        parent
    }

    pub(crate) fn lower_primitive_instantiation(
        &mut self,
        inst: ast::PrimitiveInstantiation,
    ) -> InstantiationId {
        let parent = self.reserve_instantiation(inst);
        let module_name = lower_ident_opt(inst.type_());
        let param_assigns = SmallVec::new();
        let instances =
            inst.instances().children().map(|hier| self.lower_instance(hier, parent)).collect();
        self.finish_instantiation(parent, Instantiation { module_name, param_assigns, instances });
        parent
    }

    pub(crate) fn lower_checker_instantiation(
        &mut self,
        inst: ast::CheckerInstantiation,
    ) -> InstantiationId {
        let parent = self.reserve_instantiation(inst);
        let module_name = lower_name(inst.type_());
        let param_assigns = self.lower_param_assign(inst.parameters());
        let instances =
            inst.instances().children().map(|hier| self.lower_instance(hier, parent)).collect();
        self.finish_instantiation(parent, Instantiation { module_name, param_assigns, instances });
        parent
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
                        ParamAssign::Ordered(self.lower_expr(assign.expr()))
                    }
                    NamedParamAssignment(assign) => {
                        let name = lower_ident_opt(assign.name());
                        let expr = assign.expr().map(|expr| self.lower_expr(expr));
                        ParamAssign::Named(name, expr)
                    }
                };

                let source = self.source_id(assign.syntax());
                let (assignments, sources) = self.parameter_assignments();
                crate::alloc_with_source_entry(assignments, sources, hir_assign, source)
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
                        let expr = self.lower_property_expr(conn.expr());
                        PortConn::Ordered(expr)
                    }
                    NamedPortConnection(conn) => {
                        let name = lower_ident_opt(conn.name());
                        let expr = conn.expr().map(|expr| self.lower_property_expr(expr));
                        PortConn::Named(name, expr)
                    }
                    WildcardPortConnection(_) => PortConn::Wildcard,
                };
                let source = self.source_id(conn.syntax());
                let (connections, sources) = self.port_connections();
                crate::alloc_with_source_entry(connections, sources, hir_conn, source)
            })
            .collect();

        let (name, dimensions) = instance
            .decl()
            .map(|decl| {
                let name = lower_ident_opt(decl.name());
                let dimensions =
                    decl.dimensions().children().map(|dim| self.lower_dimension(dim)).collect();
                (name, dimensions)
            })
            .unwrap_or_default();

        let data = Instance { name, dimensions, connections, parent };
        let source = self.source_id(instance.syntax());
        let (instances, sources) = self.instances();
        crate::alloc_with_source_entry(instances, sources, data, source)
    }
}

fn lower_name(name: ast::Name<'_>) -> Option<Ident> {
    lower_ident_opt(rightmost_name_token(name))
}

fn rightmost_name_token(name: ast::Name<'_>) -> Option<SyntaxToken<'_>> {
    match name {
        ast::Name::IdentifierName(name) => name.identifier(),
        ast::Name::IdentifierSelectName(name) => name.identifier(),
        ast::Name::ScopedName(name) => rightmost_name_token(name.right()),
        _ => None,
    }
}
