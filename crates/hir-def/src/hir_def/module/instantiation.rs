use la_arena::Idx;
use smallvec::SmallVec;
use syntax::{SyntaxToken, ast};

use crate::{
    hir_def::{
        Ident, alloc_with_source,
        expr::{ExprId, data_ty::Dimension},
        lower::{LoweringCtx, ModuleItemStore},
        lower_ident_opt,
    },
    source_map::{
        AstId, AstKind, FromSourceAst, IsSrc, NamedAstId, SourceAst, ToAstNode,
        exact_ast_node_from_ptr,
    },
};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Instantiation {
    pub module_name: Option<Ident>,
    pub param_assigns: SmallVec<[ParamAssignId; 1]>,
    pub instances: SmallVec<[InstanceId; 1]>,
}

pub type InstantiationId = Idx<Instantiation>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct HierarchyInstantiationAst;

impl AstKind for HierarchyInstantiationAst {
    type Node<'a> = ast::HierarchyInstantiation<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PrimitiveInstantiationAst;

impl AstKind for PrimitiveInstantiationAst {
    type Node<'a> = ast::PrimitiveInstantiation<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct CheckerInstantiationAst;

impl AstKind for CheckerInstantiationAst {
    type Node<'a> = ast::CheckerInstantiation<'a>;
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum InstantiationSrc {
    HierarchyInstantiation(AstId<HierarchyInstantiationAst>),
    PrimitiveInstantiation(AstId<PrimitiveInstantiationAst>),
    CheckerInstantiation(AstId<CheckerInstantiationAst>),
}

impl IsSrc for InstantiationSrc {
    fn kind(&self) -> syntax::SyntaxKind {
        syntax::ptr::SyntaxNodePtr::from(*self).kind()
    }

    fn range(&self) -> utils::text_edit::TextRange {
        syntax::ptr::SyntaxNodePtr::from(*self).range()
    }
}

impl<'a> ToAstNode<'a, ast::HierarchyInstantiation<'a>> for InstantiationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::HierarchyInstantiation<'a>> {
        let InstantiationSrc::HierarchyInstantiation(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::PrimitiveInstantiation<'a>> for InstantiationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::PrimitiveInstantiation<'a>> {
        let InstantiationSrc::PrimitiveInstantiation(src) = self else { return None };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> ToAstNode<'a, ast::CheckerInstantiation<'a>> for InstantiationSrc {
    fn to_node(&self, tree: &'a syntax::SyntaxTree) -> Option<ast::CheckerInstantiation<'a>> {
        let InstantiationSrc::CheckerInstantiation(src) = self else {
            return None;
        };
        exact_ast_node_from_ptr(src.ptr(), tree)
    }
}

impl<'a> FromSourceAst<'a, ast::HierarchyInstantiation<'a>> for InstantiationSrc {
    fn from_source_ast(node: SourceAst<ast::HierarchyInstantiation<'a>>) -> Self {
        Self::HierarchyInstantiation(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::PrimitiveInstantiation<'a>> for InstantiationSrc {
    fn from_source_ast(node: SourceAst<ast::PrimitiveInstantiation<'a>>) -> Self {
        Self::PrimitiveInstantiation(AstId::from_source_ast(node))
    }
}

impl<'a> FromSourceAst<'a, ast::CheckerInstantiation<'a>> for InstantiationSrc {
    fn from_source_ast(node: SourceAst<ast::CheckerInstantiation<'a>>) -> Self {
        Self::CheckerInstantiation(AstId::from_source_ast(node))
    }
}

impl From<InstantiationSrc> for syntax::ptr::SyntaxNodePtr {
    fn from(src: InstantiationSrc) -> Self {
        match src {
            InstantiationSrc::HierarchyInstantiation(src) => src.ptr(),
            InstantiationSrc::PrimitiveInstantiation(src) => src.ptr(),
            InstantiationSrc::CheckerInstantiation(src) => src.ptr(),
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct HierarchicalInstanceAst;

impl AstKind for HierarchicalInstanceAst {
    type Node<'a> = ast::HierarchicalInstance<'a>;
}

pub type InstanceSrc = NamedAstId<HierarchicalInstanceAst>;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ParamAssign {
    Ordered(ExprId),
    Named(Option<Ident>, Option<ExprId>),
}

pub type ParamAssignId = Idx<ParamAssign>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct ParamAssignmentAst;

impl AstKind for ParamAssignmentAst {
    type Node<'a> = ast::ParamAssignment<'a>;
}

pub type ParamAssignSrc = NamedAstId<ParamAssignmentAst>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum PortConn {
    Empty,
    Ordered(ExprId),
    Named(Option<Ident>, Option<ExprId>), // .a(b) or .a or .(b)
    Wildcard,
}

pub type PortConnId = Idx<PortConn>;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct PortConnectionAst;

impl AstKind for PortConnectionAst {
    type Node<'a> = ast::PortConnection<'a>;
}

pub type PortConnSrc = NamedAstId<PortConnectionAst>;

impl<Store: ModuleItemStore> LoweringCtx<'_, Store> {
    fn reserve_instantiation<'ast, Ast>(&mut self, ast: Ast) -> InstantiationId
    where
        Ast: syntax::ast::AstNode<'ast>,
        InstantiationSrc: FromSourceAst<'ast, Ast>,
    {
        let file_id = self.file_id;
        let (instantiations, sources) = self.instantiations();
        alloc_with_source(file_id, instantiations, sources, Instantiation::default(), ast)
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

                let file_id = self.file_id;
                let (assignments, sources) = self.parameter_assignments();
                alloc_with_source(file_id, assignments, sources, hir_assign, assign)
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
                let file_id = self.file_id;
                let (connections, sources) = self.port_connections();
                alloc_with_source(file_id, connections, sources, hir_conn, conn)
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
        let file_id = self.file_id;
        let (instances, sources) = self.instances();
        alloc_with_source(file_id, instances, sources, data, instance)
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
