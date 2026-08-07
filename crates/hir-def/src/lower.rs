use la_arena::Arena;
use preproc_expand::file::HirFileId;
use syntax::{SyntaxNode, SyntaxTree};
use triomphe::Arc;

use super::{
    body::{Body, BodySourceMap},
    checker::CheckerDef,
    declaration::Declaration,
    expr::{Expr, declarator::Declarator, timing_control::EventExpr},
    module::{
        continuous_assign::ContAssign,
        defparam::DefParam,
        instantiation::{Instance, Instantiation, ParamAssign, PortConn},
    },
    proc::Proc,
    stmt::Stmt,
    ty::NetKind,
};
use crate::{
    ast_id_map::AstIdMap,
    db::HirDefDb,
    owner::{OwnerId, OwnerKind, OwnerTable},
    source_map::{LoweringDiagnostic, LoweringDiagnosticKind, SourceMap},
};

/// Shared syntax inputs for the structural owner stores built by `ItemTree`.
/// Reusing one parse, AST map, and owner table keeps discovery and lowering on
/// the same canonical source identities and prevents query cycles.
#[derive(Clone)]
pub(crate) struct LoweringSyntax {
    pub(crate) file_id: HirFileId,
    pub(crate) tree: SyntaxTree,
    pub(crate) ast_ids: Arc<AstIdMap>,
    pub(crate) owners: Arc<OwnerTable>,
}

impl LoweringSyntax {
    pub(crate) fn new(
        file_id: HirFileId,
        tree: SyntaxTree,
        ast_ids: Arc<AstIdMap>,
        owners: Arc<OwnerTable>,
    ) -> Self {
        Self { file_id, tree, ast_ids, owners }
    }

    pub(crate) fn for_owner(db: &dyn HirDefDb, owner: OwnerId) -> Self {
        let file_id = owner.file(db);
        Self::new(file_id, db.parse(file_id), db.ast_id_map(file_id), db.owner_table(file_id))
    }
}
/// Mutable HIR/source pair for one canonical owner lowering pass.
pub(crate) struct BodyStore<'a> {
    pub(crate) data: &'a mut Body,
    pub(crate) sources: &'a mut BodySourceMap,
}

/// Store interface shared by expression, declarator, statement, and declaration
/// lowering.
pub(crate) trait LoweringStore {
    fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<Expr>);
    fn event_expressions(&mut self) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExpr>);
    fn declarators(&mut self) -> (&mut Arena<Declarator>, &mut SourceMap<Declarator>);
    fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<Stmt>);
    fn declarations(&mut self) -> (&mut Arena<Declaration>, &mut SourceMap<Declaration>);
    fn body(&mut self) -> (&mut Body, &mut BodySourceMap);
}

macro_rules! impl_lowering_store {
    ($($store:ty),+ $(,)?) => {$ (
        impl LoweringStore for $store {
            fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<Expr>) {
                (&mut self.data.exprs, &mut self.sources.expr_srcs)
            }

            fn event_expressions(&mut self) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExpr>) {
                (&mut self.data.event_exprs, &mut self.sources.event_expr_srcs)
            }

            fn declarators(&mut self) -> (&mut Arena<Declarator>, &mut SourceMap<Declarator>) {
                (&mut self.data.decls, &mut self.sources.decl_srcs)
            }

            fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<Stmt>) {
                (&mut self.data.stmts, &mut self.sources.stmt_srcs)
            }

            fn declarations(&mut self) -> (&mut Arena<Declaration>, &mut SourceMap<Declaration>) {
                (&mut self.data.declarations, &mut self.sources.declaration_srcs)
            }

            fn body(&mut self) -> (&mut Body, &mut BodySourceMap) {
                (self.data, self.sources)
            }
        }
    )+};
}

impl_lowering_store!(BodyStore<'_>);

pub(crate) trait CheckerStore: LoweringStore {
    fn checkers(&mut self) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerDef>);
}

macro_rules! impl_checker_store {
    ($($store:ty),+ $(,)?) => {$ (
        impl CheckerStore for $store {
            fn checkers(&mut self) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerDef>) {
                (&mut self.data.checkers, &mut self.sources.checker_srcs)
            }
        }
    )+};
}

impl_checker_store!(BodyStore<'_>);
pub(crate) trait ProcStore: LoweringStore {
    fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<Proc>);
}

macro_rules! impl_proc_store {
    ($($store:ty),+ $(,)?) => {$ (
        impl ProcStore for $store {
            fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<Proc>) {
                (&mut self.data.procs, &mut self.sources.proc_srcs)
            }
        }
    )+};
}

impl_proc_store!(BodyStore<'_>);

pub(crate) trait ModuleItemStore: LoweringStore {
    fn continuous_assigns(&mut self) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssign>);
    fn defparams(&mut self) -> (&mut Arena<DefParam>, &mut SourceMap<DefParam>);
    fn instantiations(&mut self) -> (&mut Arena<Instantiation>, &mut SourceMap<Instantiation>);
    fn parameter_assignments(&mut self) -> (&mut Arena<ParamAssign>, &mut SourceMap<ParamAssign>);
    fn instances(&mut self) -> (&mut Arena<Instance>, &mut SourceMap<Instance>);
    fn port_connections(&mut self) -> (&mut Arena<PortConn>, &mut SourceMap<PortConn>);
}

macro_rules! impl_module_item_store {
    ($($store:ty),+ $(,)?) => {$ (
        impl ModuleItemStore for $store {
            fn continuous_assigns(&mut self) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssign>) {
                (&mut self.data.cont_assigns, &mut self.sources.assign_srcs)
            }

            fn defparams(&mut self) -> (&mut Arena<DefParam>, &mut SourceMap<DefParam>) {
                (&mut self.data.defparams, &mut self.sources.defparam_srcs)
            }

            fn instantiations(&mut self) -> (&mut Arena<Instantiation>, &mut SourceMap<Instantiation>) {
                (&mut self.data.instantiations, &mut self.sources.instantiation_srcs)
            }

            fn parameter_assignments(&mut self) -> (&mut Arena<ParamAssign>, &mut SourceMap<ParamAssign>) {
                (&mut self.data.inst_param_assigns, &mut self.sources.inst_param_assign_srcs)
            }

            fn instances(&mut self) -> (&mut Arena<Instance>, &mut SourceMap<Instance>) {
                (&mut self.data.instances, &mut self.sources.instance_srcs)
            }

            fn port_connections(&mut self) -> (&mut Arena<PortConn>, &mut SourceMap<PortConn>) {
                (&mut self.data.inst_port_conns, &mut self.sources.inst_port_conn_srcs)
            }
        }
    )+};
}

impl_module_item_store!(BodyStore<'_>);

/// Complete mutable state for one HIR lowering pass.
pub(crate) struct LoweringCtx<Store> {
    pub(crate) file_id: HirFileId,
    pub(crate) owner: OwnerId,
    pub(crate) ast_ids: Arc<AstIdMap>,
    owners: Arc<OwnerTable>,
    pub(crate) tree: SyntaxTree,
    scope_stack: Vec<OwnerId>,
    pub(crate) store: Store,
    pub(crate) diagnostics: Vec<LoweringDiagnostic>,
    pub(crate) default_net_type: NetKind,
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn new(db: &dyn HirDefDb, owner: OwnerId, store: Store) -> Self {
        let file_id = owner.file(db);
        let syntax = LoweringSyntax::new(
            file_id,
            db.parse(file_id),
            db.ast_id_map(file_id),
            db.owner_table(file_id),
        );
        Self::new_with_syntax(owner, &syntax, store)
    }

    pub(crate) fn new_with_syntax(owner: OwnerId, syntax: &LoweringSyntax, store: Store) -> Self {
        let mut this = Self {
            file_id: syntax.file_id,
            owner,
            ast_ids: Arc::clone(&syntax.ast_ids),
            owners: Arc::clone(&syntax.owners),
            tree: syntax.tree.clone(),
            scope_stack: vec![owner],
            store,
            diagnostics: Vec::new(),
            default_net_type: NetKind::Wire,
        };
        this.store.body().0.scope_graph.ensure_root(owner);
        this
    }

    pub(crate) fn owner_for_node(&self, node: SyntaxNode<'_>, kind: OwnerKind) -> Option<OwnerId> {
        let ast_id = self.ast_ids.id_of_node_in_tree(&self.tree, node)?;
        self.owners.owner_by_ast(ast_id, kind)
    }

    pub(crate) fn source_id(&self, node: SyntaxNode<'_>) -> crate::ast_id_map::SourceAstId {
        self.ast_ids
            .id_of_node_in_tree(&self.tree, node)
            .expect("every lowered AST node must have a canonical source identity")
    }

    pub(crate) fn current_scope(&self) -> OwnerId {
        *self.scope_stack.last().expect("body lowering always has a root scope")
    }

    pub(crate) fn current_owner(&self) -> OwnerId {
        self.current_scope()
    }

    pub(crate) fn enter_body_scope(&mut self, owner: OwnerId) {
        let parent = self.current_scope();
        let graph_parent = self.owners.owner(owner).and_then(|data| data.parent);
        assert_eq!(graph_parent, Some(parent), "body scope must follow the canonical owner graph");
        self.store.body().0.scope_graph.insert(owner, Some(parent));
        self.scope_stack.push(owner);
    }

    pub(crate) fn leave_body_scope(&mut self, owner: OwnerId) {
        assert_eq!(self.scope_stack.pop(), Some(owner), "body scopes must leave in LIFO order");
        assert!(!self.scope_stack.is_empty(), "cannot leave the body root scope");
    }

    pub(crate) fn push_body_item(&mut self, item: crate::block::BlockItem) {
        let scope = self.current_scope();
        self.store.body().0.scope_graph.push_item(scope, item);
    }

    pub(crate) fn record_body_declaration(
        &mut self,
        declaration: crate::declaration::DeclarationId,
    ) {
        self.push_body_item(crate::block::BlockItem::DeclarationId(declaration));
    }

    pub(crate) fn record_body_declarator(&mut self, declarator: crate::expr::declarator::DeclId) {
        let scope = self.current_scope();
        self.store.body().0.scope_graph.push_declarator(scope, declarator);
    }

    pub(crate) fn record_body_typedef(&mut self, typedef: crate::typedef::TypedefId) {
        let scope = self.current_scope();
        let body = self.store.body().0;
        body.scope_graph.push_typedef(scope, typedef);
        body.scope_graph.push_item(scope, crate::block::BlockItem::TypedefId(typedef));
    }

    pub(crate) fn record_body_statement(&mut self, statement: crate::stmt::StmtId) {
        let scope = self.current_scope();
        self.store.body().0.scope_graph.push_statement(scope, statement);
    }

    pub(crate) fn report_unsupported(&mut self, node: SyntaxNode<'_>, message: &'static str) {
        self.diagnostics.push(LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind: node.kind(),
            source: Some(self.source_id(node)),
            range: None,
            message,
        });
    }

    pub(crate) fn report_invalid(&mut self, node: SyntaxNode<'_>, message: &'static str) {
        self.diagnostics.push(LoweringDiagnostic {
            kind: LoweringDiagnosticKind::InvalidSyntax,
            syntax_kind: node.kind(),
            source: Some(self.source_id(node)),
            range: None,
            message,
        });
    }

    pub(crate) fn emit_diagnostics(&mut self) -> Vec<LoweringDiagnostic> {
        let diagnostics = std::mem::take(&mut self.diagnostics);
        for diagnostic in &diagnostics {
            tracing::warn!(
                file = ?self.file_id,
                owner = ?self.owner,
                syntax_kind = ?diagnostic.syntax_kind,
                source = ?diagnostic.source,
                message = diagnostic.message,
                "HIR lowering diagnostic"
            );
        }
        diagnostics
    }
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<Expr>) {
        self.store.expressions()
    }

    pub(crate) fn event_expressions(
        &mut self,
    ) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExpr>) {
        self.store.event_expressions()
    }

    pub(crate) fn declarators(&mut self) -> (&mut Arena<Declarator>, &mut SourceMap<Declarator>) {
        self.store.declarators()
    }

    pub(crate) fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<Stmt>) {
        self.store.statements()
    }

    pub(crate) fn declarations(
        &mut self,
    ) -> (&mut Arena<Declaration>, &mut SourceMap<Declaration>) {
        self.store.declarations()
    }
}

impl<Store: ProcStore> LoweringCtx<Store> {
    pub(crate) fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<Proc>) {
        self.store.procs()
    }
}

impl<Store: ModuleItemStore> LoweringCtx<Store> {
    pub(crate) fn continuous_assigns(
        &mut self,
    ) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssign>) {
        self.store.continuous_assigns()
    }

    pub(crate) fn defparams(&mut self) -> (&mut Arena<DefParam>, &mut SourceMap<DefParam>) {
        self.store.defparams()
    }

    pub(crate) fn instantiations(
        &mut self,
    ) -> (&mut Arena<Instantiation>, &mut SourceMap<Instantiation>) {
        self.store.instantiations()
    }

    pub(crate) fn parameter_assignments(
        &mut self,
    ) -> (&mut Arena<ParamAssign>, &mut SourceMap<ParamAssign>) {
        self.store.parameter_assignments()
    }

    pub(crate) fn instances(&mut self) -> (&mut Arena<Instance>, &mut SourceMap<Instance>) {
        self.store.instances()
    }

    pub(crate) fn port_connections(&mut self) -> (&mut Arena<PortConn>, &mut SourceMap<PortConn>) {
        self.store.port_connections()
    }
}
