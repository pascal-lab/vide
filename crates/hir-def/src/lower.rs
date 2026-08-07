use la_arena::Arena;
use preproc_expand::file::HirFileId;
use syntax::{SyntaxKind, SyntaxNode, SyntaxTree};
use triomphe::Arc;
use utils::text_edit::TextRange;

use super::{
    body::{Body, BodySourceMap},
    checker::{CheckerDef, CheckerSrc},
    declaration::{Declaration, DeclarationSrc},
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprSrc},
    },
    file::{FileSourceMap, HirFile},
    module::{
        Module, ModuleSourceMap,
        continuous_assign::{ContAssign, ContAssignSrc},
        defparam::{DefParam, DefParamSrc},
        generate::{GenerateBlock, GenerateBlockSourceMap},
        instantiation::{
            Instance, InstanceSrc, Instantiation, InstantiationSrc, ParamAssign, ParamAssignSrc,
            PortConn, PortConnSrc,
        },
    },
    proc::{Proc, ProcSrc},
    stmt::{Stmt, StmtSrc},
    ty::NetKind,
};
use crate::{
    ast_id_map::AstIdMap,
    container::ArenaOwnerId,
    db::HirDefDb,
    owner::{OwnerId, OwnerKind, OwnerTable},
    region_tree::RegionTreeBuilder,
    source_map::{LoweringDiagnostic, LoweringDiagnosticKind, SourceMap},
};

/// Mutable data/source pair for a file lowering pass.
pub(crate) struct FileStore<'a> {
    pub(crate) data: &'a mut HirFile,
    pub(crate) sources: &'a mut FileSourceMap,
    pub(crate) body: &'a mut Body,
    pub(crate) body_sources: &'a mut BodySourceMap,
}

/// Mutable structural data plus the owner-local semantic store for a module.
pub(crate) struct ModuleStore<'a> {
    pub(crate) data: &'a mut Module,
    pub(crate) sources: &'a mut ModuleSourceMap,
    pub(crate) body: &'a mut Body,
    pub(crate) body_sources: &'a mut BodySourceMap,
}

/// Mutable structural data plus the owner-local semantic store for a generate
/// block.
pub(crate) struct GenerateBlockStore<'a> {
    pub(crate) data: &'a mut GenerateBlock,
    pub(crate) sources: &'a mut GenerateBlockSourceMap,
    pub(crate) body: &'a mut Body,
    pub(crate) body_sources: &'a mut BodySourceMap,
}

/// Mutable data/source pair for a canonical executable body.
pub(crate) struct BodyStore<'a> {
    pub(crate) data: &'a mut Body,
    pub(crate) sources: &'a mut BodySourceMap,
}

/// Store interface shared by expression, declarator, statement, and declaration
/// lowering.
pub(crate) trait LoweringStore {
    fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<ExprSrc, Expr>);
    fn event_expressions(
        &mut self,
    ) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExprSrc, EventExpr>);
    fn declarators(
        &mut self,
    ) -> (&mut Arena<Declarator>, &mut SourceMap<DeclaratorSrc, Declarator>);
    fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<StmtSrc, Stmt>);
    fn declarations(
        &mut self,
    ) -> (&mut Arena<Declaration>, &mut SourceMap<DeclarationSrc, Declaration>);
    fn body(&mut self) -> (&mut Body, &mut BodySourceMap);
}

macro_rules! impl_owner_lowering_store {
    ($store:ty) => {
        impl LoweringStore for $store {
            fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<ExprSrc, Expr>) {
                (&mut self.body.exprs, &mut self.body_sources.expr_srcs)
            }

            fn event_expressions(
                &mut self,
            ) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExprSrc, EventExpr>) {
                (&mut self.body.event_exprs, &mut self.body_sources.event_expr_srcs)
            }

            fn declarators(
                &mut self,
            ) -> (&mut Arena<Declarator>, &mut SourceMap<DeclaratorSrc, Declarator>) {
                (&mut self.body.decls, &mut self.body_sources.decl_srcs)
            }

            fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<StmtSrc, Stmt>) {
                (&mut self.body.stmts, &mut self.body_sources.stmt_srcs)
            }

            fn declarations(
                &mut self,
            ) -> (&mut Arena<Declaration>, &mut SourceMap<DeclarationSrc, Declaration>) {
                (&mut self.body.declarations, &mut self.body_sources.declaration_srcs)
            }

            fn body(&mut self) -> (&mut Body, &mut BodySourceMap) {
                (self.body, self.body_sources)
            }
        }
    };
}

impl_owner_lowering_store!(FileStore<'_>);
impl_owner_lowering_store!(ModuleStore<'_>);
impl_owner_lowering_store!(GenerateBlockStore<'_>);

impl LoweringStore for BodyStore<'_> {
    fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<ExprSrc, Expr>) {
        (&mut self.data.exprs, &mut self.sources.expr_srcs)
    }

    fn event_expressions(
        &mut self,
    ) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExprSrc, EventExpr>) {
        (&mut self.data.event_exprs, &mut self.sources.event_expr_srcs)
    }

    fn declarators(
        &mut self,
    ) -> (&mut Arena<Declarator>, &mut SourceMap<DeclaratorSrc, Declarator>) {
        (&mut self.data.decls, &mut self.sources.decl_srcs)
    }

    fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<StmtSrc, Stmt>) {
        (&mut self.data.stmts, &mut self.sources.stmt_srcs)
    }

    fn declarations(
        &mut self,
    ) -> (&mut Arena<Declaration>, &mut SourceMap<DeclarationSrc, Declaration>) {
        (&mut self.data.declarations, &mut self.sources.declaration_srcs)
    }

    fn body(&mut self) -> (&mut Body, &mut BodySourceMap) {
        (self.data, self.sources)
    }
}

pub(crate) trait CheckerStore: LoweringStore {
    fn checkers(&mut self) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerSrc, CheckerDef>);
}

macro_rules! impl_checker_store {
    ($store:ty) => {
        impl CheckerStore for $store {
            fn checkers(
                &mut self,
            ) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerSrc, CheckerDef>) {
                (&mut self.data.checkers, &mut self.sources.checker_srcs)
            }
        }
    };
}

impl_checker_store!(FileStore<'_>);
impl_checker_store!(ModuleStore<'_>);

pub(crate) trait ProcStore: LoweringStore {
    fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>);
}

macro_rules! impl_proc_store {
    ($store:ty) => {
        impl ProcStore for $store {
            fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>) {
                (&mut self.data.procs, &mut self.sources.proc_srcs)
            }
        }
    };
}

impl_proc_store!(FileStore<'_>);
impl_proc_store!(ModuleStore<'_>);
impl_proc_store!(GenerateBlockStore<'_>);

pub(crate) trait ModuleItemStore: LoweringStore {
    fn continuous_assigns(
        &mut self,
    ) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssignSrc, ContAssign>);
    fn defparams(&mut self) -> (&mut Arena<DefParam>, &mut SourceMap<DefParamSrc, DefParam>);
    fn instantiations(
        &mut self,
    ) -> (&mut Arena<Instantiation>, &mut SourceMap<InstantiationSrc, Instantiation>);
    fn parameter_assignments(
        &mut self,
    ) -> (&mut Arena<ParamAssign>, &mut SourceMap<ParamAssignSrc, ParamAssign>);
    fn instances(&mut self) -> (&mut Arena<Instance>, &mut SourceMap<InstanceSrc, Instance>);
    fn port_connections(&mut self)
    -> (&mut Arena<PortConn>, &mut SourceMap<PortConnSrc, PortConn>);
}

macro_rules! impl_module_item_store {
    ($store:ty) => {
        impl ModuleItemStore for $store {
            fn continuous_assigns(
                &mut self,
            ) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssignSrc, ContAssign>) {
                (&mut self.data.cont_assigns, &mut self.sources.assign_srcs)
            }

            fn defparams(
                &mut self,
            ) -> (&mut Arena<DefParam>, &mut SourceMap<DefParamSrc, DefParam>) {
                (&mut self.data.defparams, &mut self.sources.defparam_srcs)
            }

            fn instantiations(
                &mut self,
            ) -> (&mut Arena<Instantiation>, &mut SourceMap<InstantiationSrc, Instantiation>) {
                (&mut self.data.instantiations, &mut self.sources.instantiation_srcs)
            }

            fn parameter_assignments(
                &mut self,
            ) -> (&mut Arena<ParamAssign>, &mut SourceMap<ParamAssignSrc, ParamAssign>) {
                (&mut self.data.inst_param_assigns, &mut self.sources.inst_param_assign_srcs)
            }

            fn instances(
                &mut self,
            ) -> (&mut Arena<Instance>, &mut SourceMap<InstanceSrc, Instance>) {
                (&mut self.data.instances, &mut self.sources.instance_srcs)
            }

            fn port_connections(
                &mut self,
            ) -> (&mut Arena<PortConn>, &mut SourceMap<PortConnSrc, PortConn>) {
                (&mut self.data.inst_port_conns, &mut self.sources.inst_port_conn_srcs)
            }
        }
    };
}

impl_module_item_store!(ModuleStore<'_>);
impl_module_item_store!(GenerateBlockStore<'_>);


/// Complete mutable state for one HIR lowering pass.
pub(crate) struct LoweringCtx<Store> {
    pub(crate) file_id: HirFileId,
    pub(crate) owner: OwnerId,
    ast_ids: Arc<AstIdMap>,
    owners: Arc<OwnerTable>,
    tree: SyntaxTree,
    scope_stack: Vec<OwnerId>,
    pub(crate) store: Store,
    pub(crate) diagnostics: Vec<LoweringDiagnostic>,
    pub(crate) region_tree: RegionTreeBuilder,
    pub(crate) default_net_type: NetKind,
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn new(db: &dyn HirDefDb, owner: OwnerId, store: Store) -> Self {
        let file_id = owner.file(db);
        let tree = db.parse(file_id);
        let mut this = Self {
            file_id,
            owner,
            ast_ids: db.ast_id_map(file_id),
            owners: db.owner_table(file_id),
            tree,
            scope_stack: vec![owner],
            store,
            diagnostics: Vec::new(),
            region_tree: RegionTreeBuilder::new(),
            default_net_type: NetKind::Wire,
        };
        this.store.body().0.scope_graph.ensure_root(owner);
        this
    }

    pub(crate) fn owner_for_node(&self, node: SyntaxNode<'_>, kind: OwnerKind) -> Option<OwnerId> {
        let ast_id = self.ast_ids.id_of_node_in_tree(&self.tree, node)?;
        self.owners.owner_by_ast(ast_id, kind)
    }

    pub(crate) fn current_scope(&self) -> OwnerId {
        *self.scope_stack.last().expect("body lowering always has a root scope")
    }

    pub(crate) fn current_arena_owner(&self) -> ArenaOwnerId {
        ArenaOwnerId::Owner(self.current_scope())
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

    pub(crate) fn set_scope_region_tree(&mut self, owner: OwnerId, regions: crate::region_tree::RegionTree) {
        self.store.body().1.insert_scope_region_tree(owner, regions);
    }

    pub(crate) fn report_unsupported(
        &mut self,
        syntax_kind: SyntaxKind,
        range: Option<TextRange>,
        message: &'static str,
    ) {
        self.diagnostics.push(LoweringDiagnostic {
            kind: LoweringDiagnosticKind::UnsupportedSyntax,
            syntax_kind,
            range,
            message,
        });
    }

    pub(crate) fn report_invalid(
        &mut self,
        syntax_kind: SyntaxKind,
        range: Option<TextRange>,
        message: &'static str,
    ) {
        self.diagnostics.push(LoweringDiagnostic {
            kind: LoweringDiagnosticKind::InvalidSyntax,
            syntax_kind,
            range,
            message,
        });
    }

    pub(crate) fn emit_diagnostics(&mut self) -> Vec<LoweringDiagnostic> {
        let diagnostics = std::mem::take(&mut self.diagnostics);
        for diagnostic in &diagnostics {
            tracing::warn!(
                file = ?self.file_id,
                owner = ?self.owner,
                kind = ?diagnostic.kind,
                syntax_kind = ?diagnostic.syntax_kind,
                range = ?diagnostic.range,
                message = diagnostic.message,
                "HIR lowering diagnostic"
            );
        }
        diagnostics
    }
}

impl<Store: LoweringStore> LoweringCtx<Store> {
    pub(crate) fn expressions(&mut self) -> (&mut Arena<Expr>, &mut SourceMap<ExprSrc, Expr>) {
        self.store.expressions()
    }

    pub(crate) fn event_expressions(
        &mut self,
    ) -> (&mut Arena<EventExpr>, &mut SourceMap<EventExprSrc, EventExpr>) {
        self.store.event_expressions()
    }

    pub(crate) fn declarators(
        &mut self,
    ) -> (&mut Arena<Declarator>, &mut SourceMap<DeclaratorSrc, Declarator>) {
        self.store.declarators()
    }

    pub(crate) fn statements(&mut self) -> (&mut Arena<Stmt>, &mut SourceMap<StmtSrc, Stmt>) {
        self.store.statements()
    }

    pub(crate) fn declarations(
        &mut self,
    ) -> (&mut Arena<Declaration>, &mut SourceMap<DeclarationSrc, Declaration>) {
        self.store.declarations()
    }
}

impl<Store: ProcStore> LoweringCtx<Store> {
    pub(crate) fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>) {
        self.store.procs()
    }
}

impl<Store: ModuleItemStore> LoweringCtx<Store> {
    pub(crate) fn continuous_assigns(
        &mut self,
    ) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssignSrc, ContAssign>) {
        self.store.continuous_assigns()
    }

    pub(crate) fn defparams(
        &mut self,
    ) -> (&mut Arena<DefParam>, &mut SourceMap<DefParamSrc, DefParam>) {
        self.store.defparams()
    }

    pub(crate) fn instantiations(
        &mut self,
    ) -> (&mut Arena<Instantiation>, &mut SourceMap<InstantiationSrc, Instantiation>) {
        self.store.instantiations()
    }

    pub(crate) fn parameter_assignments(
        &mut self,
    ) -> (&mut Arena<ParamAssign>, &mut SourceMap<ParamAssignSrc, ParamAssign>) {
        self.store.parameter_assignments()
    }

    pub(crate) fn instances(
        &mut self,
    ) -> (&mut Arena<Instance>, &mut SourceMap<InstanceSrc, Instance>) {
        self.store.instances()
    }

    pub(crate) fn port_connections(
        &mut self,
    ) -> (&mut Arena<PortConn>, &mut SourceMap<PortConnSrc, PortConn>) {
        self.store.port_connections()
    }
}
