use la_arena::Arena;
use syntax::SyntaxKind;
use utils::text_edit::TextRange;

use super::{
    block::{Block, BlockId, BlockSourceMap},
    checker::{CheckerDef, CheckerSrc},
    declaration::{Declaration, DeclarationSrc},
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprSrc},
    },
    file::{FileSourceMap, HirFile},
    module::{
        Module, ModuleId, ModuleSourceMap,
        continuous_assgin::{ContAssign, ContAssignSrc},
        defparam::{DefParam, DefParamSrc},
        generate::{GenerateBlock, GenerateBlockId, GenerateBlockSourceMap},
        instantiation::{
            Instance, InstanceSrc, Instantiation, InstantiationSrc, ParamAssign, ParamAssignSrc,
            PortConn, PortConnSrc,
        },
    },
    proc::{Proc, ProcSrc},
    stmt::{Stmt, StmtSrc},
    subroutine::{Subroutine, SubroutineSourceMap},
    ty::NetKind,
};
use crate::{
    container::ScopeId, db::InternDb, file::HirFileId, region_tree::RegionTreeBuilder,
    source_map::SourceMap,
};

/// Mutable data/source pair for a file lowering pass.
pub(crate) struct FileStore<'a> {
    pub(crate) data: &'a mut HirFile,
    pub(crate) sources: &'a mut FileSourceMap,
}

/// Mutable data/source pair for a module lowering pass.
pub(crate) struct ModuleStore<'a> {
    pub(crate) data: &'a mut Module,
    pub(crate) sources: &'a mut ModuleSourceMap,
}

/// Mutable data/source pair for a generate-block lowering pass.
pub(crate) struct GenerateBlockStore<'a> {
    pub(crate) data: &'a mut GenerateBlock,
    pub(crate) sources: &'a mut GenerateBlockSourceMap,
}

/// Mutable data/source pair for a procedural-block lowering pass.
pub(crate) struct BlockStore<'a> {
    pub(crate) data: &'a mut Block,
    pub(crate) sources: &'a mut BlockSourceMap,
}

/// Mutable data/source pair for a subroutine-body lowering pass.
pub(crate) struct SubroutineStore<'a> {
    pub(crate) data: &'a mut Subroutine,
    pub(crate) sources: &'a mut SubroutineSourceMap,
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
}

impl LoweringStore for FileStore<'_> {
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
}

impl LoweringStore for ModuleStore<'_> {
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
}

impl LoweringStore for GenerateBlockStore<'_> {
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
}

impl LoweringStore for BlockStore<'_> {
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
}

impl LoweringStore for SubroutineStore<'_> {
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
}

pub(crate) trait CheckerStore: LoweringStore {
    fn checkers(&mut self) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerSrc, CheckerDef>);
}

impl CheckerStore for FileStore<'_> {
    fn checkers(&mut self) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerSrc, CheckerDef>) {
        (&mut self.data.checkers, &mut self.sources.checker_srcs)
    }
}

impl CheckerStore for ModuleStore<'_> {
    fn checkers(&mut self) -> (&mut Arena<CheckerDef>, &mut SourceMap<CheckerSrc, CheckerDef>) {
        (&mut self.data.checkers, &mut self.sources.checker_srcs)
    }
}

pub(crate) trait ProcStore: LoweringStore {
    fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>);
}

impl ProcStore for FileStore<'_> {
    fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>) {
        (&mut self.data.procs, &mut self.sources.proc_srcs)
    }
}

impl ProcStore for ModuleStore<'_> {
    fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>) {
        (&mut self.data.procs, &mut self.sources.proc_srcs)
    }
}

impl ProcStore for GenerateBlockStore<'_> {
    fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>) {
        (&mut self.data.procs, &mut self.sources.proc_srcs)
    }
}

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

impl ModuleItemStore for ModuleStore<'_> {
    fn continuous_assigns(
        &mut self,
    ) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssignSrc, ContAssign>) {
        (&mut self.data.cont_assigns, &mut self.sources.assign_srcs)
    }

    fn defparams(&mut self) -> (&mut Arena<DefParam>, &mut SourceMap<DefParamSrc, DefParam>) {
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

    fn instances(&mut self) -> (&mut Arena<Instance>, &mut SourceMap<InstanceSrc, Instance>) {
        (&mut self.data.instances, &mut self.sources.instance_srcs)
    }

    fn port_connections(
        &mut self,
    ) -> (&mut Arena<PortConn>, &mut SourceMap<PortConnSrc, PortConn>) {
        (&mut self.data.inst_port_conns, &mut self.sources.inst_port_conn_srcs)
    }
}

impl ModuleItemStore for GenerateBlockStore<'_> {
    fn continuous_assigns(
        &mut self,
    ) -> (&mut Arena<ContAssign>, &mut SourceMap<ContAssignSrc, ContAssign>) {
        (&mut self.data.cont_assigns, &mut self.sources.assign_srcs)
    }

    fn defparams(&mut self) -> (&mut Arena<DefParam>, &mut SourceMap<DefParamSrc, DefParam>) {
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

    fn instances(&mut self) -> (&mut Arena<Instance>, &mut SourceMap<InstanceSrc, Instance>) {
        (&mut self.data.instances, &mut self.sources.instance_srcs)
    }

    fn port_connections(
        &mut self,
    ) -> (&mut Arena<PortConn>, &mut SourceMap<PortConnSrc, PortConn>) {
        (&mut self.data.inst_port_conns, &mut self.sources.inst_port_conn_srcs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LoweringDiagnostic {
    pub(crate) kind: SyntaxKind,
    pub(crate) range: Option<TextRange>,
    pub(crate) message: &'static str,
}

/// Complete mutable state for one HIR lowering pass.
pub(crate) struct LoweringCtx<'a, Store> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) owner: ScopeId,
    pub(crate) store: Store,
    pub(crate) diagnostics: Vec<LoweringDiagnostic>,
    pub(crate) region_tree: RegionTreeBuilder,
    pub(crate) default_net_type: NetKind,
}

impl<'a, Store> LoweringCtx<'a, Store> {
    pub(crate) fn new(
        db: &'a dyn InternDb,
        file_id: HirFileId,
        owner: ScopeId,
        store: Store,
    ) -> Self {
        Self {
            db,
            file_id,
            owner,
            store,
            diagnostics: Vec::new(),
            region_tree: RegionTreeBuilder::new(),
            default_net_type: NetKind::Wire,
        }
    }

    pub(crate) fn module_id(&self) -> ModuleId {
        let ScopeId::Module(module_id) = self.owner else {
            unreachable!("module-only lowering called for {:?}", self.owner.kind());
        };
        module_id
    }

    pub(crate) fn generate_block_id(&self) -> GenerateBlockId {
        let ScopeId::GenerateBlock(generate_block_id) = self.owner else {
            unreachable!("generate-block-only lowering called for {:?}", self.owner.kind());
        };
        generate_block_id
    }

    pub(crate) fn block_id(&self) -> BlockId {
        let ScopeId::Block(block_id) = self.owner else {
            unreachable!("block-only lowering called for {:?}", self.owner.kind());
        };
        block_id
    }

    pub(crate) fn report_unsupported(
        &mut self,
        kind: SyntaxKind,
        range: Option<TextRange>,
        message: &'static str,
    ) {
        self.diagnostics.push(LoweringDiagnostic { kind, range, message });
    }

    pub(crate) fn emit_diagnostics(&mut self) {
        for diagnostic in self.diagnostics.drain(..) {
            tracing::warn!(
                file = ?self.file_id,
                owner = ?self.owner,
                kind = ?diagnostic.kind,
                range = ?diagnostic.range,
                message = diagnostic.message,
                "HIR lowering diagnostic"
            );
        }
    }
}

impl<Store: LoweringStore> LoweringCtx<'_, Store> {
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

impl<Store: ProcStore> LoweringCtx<'_, Store> {
    pub(crate) fn procs(&mut self) -> (&mut Arena<Proc>, &mut SourceMap<ProcSrc, Proc>) {
        self.store.procs()
    }
}

impl<Store: ModuleItemStore> LoweringCtx<'_, Store> {
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
