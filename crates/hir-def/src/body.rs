use la_arena::{Arena, Idx, IdxRange};
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    ast::{self, AstNode},
    match_ast,
};
use triomphe::Arc;

use crate::{
    PackageImport,
    aggregate::{StructDef, StructId, lower_struct_def},
    ast_id_map::SourceAstId,
    block::BlockItem,
    checker::{CheckerDef, CheckerId},
    covergroup::{CovergroupDef, CovergroupId, CoverpointDef, CoverpointId, CrossDef, CrossId},
    db::HirDefDb,
    declaration::{DataDecl, Declaration, DeclarationId},
    expr::{
        Expr, ExprId,
        declarator::{DeclId, Declarator, empty_decls_range},
        timing_control::{EventExpr, EventExprId},
    },
    file::{
        config::{ConfigDecl, ConfigDeclId},
        library::{LibraryDecl, LibraryDeclId, LibraryInclude, LibraryIncludeId},
        udp::{UdpDecl, UdpDeclId},
    },
    lower::{BodyStore, LoweringCtx, LoweringStore, LoweringSyntax},
    lower_ident_opt,
    module::{
        clocking::{ClockingBlockDef, ClockingBlockId, DefaultClockingRef},
        continuous_assign::{ContAssign, ContAssignId},
        defparam::{DefParam, DefParamId},
        generate::{GenerateBlockKind, GenerateRegion, GenerateRegionId},
        instantiation::{
            Instance, InstanceId, Instantiation, InstantiationId, ParamAssign, ParamAssignId,
            PortConn, PortConnId,
        },
        modport::{ModportDef, ModportId},
        port::{
            NonAnsiPort, NonAnsiPortId, PortDecl, PortDeclId, PortRef, PortRefId, PortSrcs, Ports,
        },
        specify::{SpecifyBlock, SpecifyBlockId, SpecifyItem, SpecifyItemId},
    },
    owner::{OwnerId, OwnerKind},
    proc::{Proc, ProcId},
    source_map::{Lowered, LoweredData, SourceMap},
    stmt::{Stmt, StmtId},
    subroutine::{Subroutine, lower_subroutine},
    typedef::{Typedef, TypedefId, lower_typedef_data_ty},
};

/// One lexical scope inside a [`Body`]. Canonical [`OwnerId`] values are the
/// only scope identities; the graph does not allocate a parallel block id.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BodyScopeData {
    owner: OwnerId,
    parent: Option<OwnerId>,
    items: SmallVec<[BlockItem; 4]>,
    declarators: SmallVec<[DeclId; 4]>,
    typedefs: SmallVec<[TypedefId; 2]>,
    statements: SmallVec<[StmtId; 4]>,
}

impl BodyScopeData {
    pub fn owner(&self) -> OwnerId {
        self.owner
    }

    pub fn parent(&self) -> Option<OwnerId> {
        self.parent
    }

    pub fn items(&self) -> &[BlockItem] {
        &self.items
    }

    pub fn declarators(&self) -> &[DeclId] {
        &self.declarators
    }

    pub fn typedefs(&self) -> &[TypedefId] {
        &self.typedefs
    }

    pub fn statements(&self) -> &[StmtId] {
        &self.statements
    }
}

/// Lexical scopes for one owner-local body, in source discovery order.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct BodyScopeGraph {
    root: Option<OwnerId>,
    scopes: Vec<BodyScopeData>,
    by_owner: FxHashMap<OwnerId, usize>,
}

impl BodyScopeGraph {
    pub fn root(&self) -> Option<OwnerId> {
        self.root
    }

    pub fn scope(&self, owner: OwnerId) -> Option<&BodyScopeData> {
        self.by_owner.get(&owner).map(|index| &self.scopes[*index])
    }

    pub fn scopes(&self) -> impl Iterator<Item = &BodyScopeData> {
        self.scopes.iter()
    }

    fn scope_mut(&mut self, owner: OwnerId) -> &mut BodyScopeData {
        let index = self.by_owner[&owner];
        &mut self.scopes[index]
    }

    pub(crate) fn ensure_root(&mut self, owner: OwnerId) {
        if let Some(root) = self.root {
            assert_eq!(root, owner, "one Body cannot have multiple root owners");
            return;
        }
        self.root = Some(owner);
        self.insert(owner, None);
    }

    pub(crate) fn insert(&mut self, owner: OwnerId, parent: Option<OwnerId>) {
        assert!(!self.by_owner.contains_key(&owner), "body scope owner inserted twice");
        if let Some(parent) = parent {
            assert!(self.by_owner.contains_key(&parent), "body scope parent must exist first");
        }
        let index = self.scopes.len();
        self.scopes.push(BodyScopeData {
            owner,
            parent,
            items: SmallVec::new(),
            declarators: SmallVec::new(),
            typedefs: SmallVec::new(),
            statements: SmallVec::new(),
        });
        self.by_owner.insert(owner, index);
    }

    pub(crate) fn push_item(&mut self, owner: OwnerId, item: BlockItem) {
        self.scope_mut(owner).items.push(item);
    }

    pub(crate) fn push_declarator(&mut self, owner: OwnerId, declarator: DeclId) {
        self.scope_mut(owner).declarators.push(declarator);
    }

    pub(crate) fn push_typedef(&mut self, owner: OwnerId, typedef: TypedefId) {
        self.scope_mut(owner).typedefs.push(typedef);
    }

    pub(crate) fn push_statement(&mut self, owner: OwnerId, statement: StmtId) {
        self.scope_mut(owner).statements.push(statement);
    }

    fn shrink_to_fit(&mut self) {
        for scope in &mut self.scopes {
            scope.items.shrink_to_fit();
            scope.declarators.shrink_to_fit();
            scope.typedefs.shrink_to_fit();
            scope.statements.shrink_to_fit();
        }
        self.scopes.shrink_to_fit();
        self.by_owner.shrink_to_fit();
    }
}

/// One lowered item in source order for any owner kind.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BodyItem {
    ModuleOwner(OwnerId),
    ProcId(ProcId),
    DeclarationId(DeclarationId),
    TypedefId(TypedefId),
    StructId(StructId),
    ConfigDeclId(ConfigDeclId),
    UdpDeclId(UdpDeclId),
    LibraryDeclId(LibraryDeclId),
    LibraryIncludeId(LibraryIncludeId),
    CheckerOwner(OwnerId),
    CovergroupOwner(OwnerId),
    ContAssignId(ContAssignId),
    DefParamId(DefParamId),
    GenerateRegionId(GenerateRegionId),
    GenerateBlockOwner(OwnerId),
    SpecifyBlockId(SpecifyBlockId),
    SpecifyItemId(SpecifyItemId),
    InstantiationId(InstantiationId),
    PortDeclId(PortDeclId),
    SubroutineOwner(OwnerId),
    ModportId(ModportId),
    ClockingBlockOwner(OwnerId),
}

macro_rules! impl_body_item_from {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl From<$ty> for BodyItem {
                fn from(value: $ty) -> Self {
                    BodyItem::$variant(value)
                }
            }
        )*
    };
}

impl_body_item_from! {
    ProcId => ProcId,
    DeclarationId => DeclarationId,
    TypedefId => TypedefId,
    StructId => StructId,
    ConfigDeclId => ConfigDeclId,
    UdpDeclId => UdpDeclId,
    LibraryDeclId => LibraryDeclId,
    LibraryIncludeId => LibraryIncludeId,
    ContAssignId => ContAssignId,
    DefParamId => DefParamId,
    GenerateRegionId => GenerateRegionId,
    SpecifyBlockId => SpecifyBlockId,
    SpecifyItemId => SpecifyItemId,
    InstantiationId => InstantiationId,
    PortDeclId => PortDeclId,
    ModportId => ModportId,
}

/// All position-free HIR owned by one canonical [`OwnerId`]. Nested lexical
/// blocks share these arenas and are partitioned by [`BodyScopeGraph`].
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Body {
    pub name: Option<crate::Ident>,
    pub items: Vec<BodyItem>,
    pub scope_graph: BodyScopeGraph,
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
    pub root_stmt: Option<StmtId>,
    pub procs: Arena<Proc>,
    pub config_decls: Arena<ConfigDecl>,
    pub udp_decls: Arena<UdpDecl>,
    pub library_decls: Arena<LibraryDecl>,
    pub library_includes: Arena<LibraryInclude>,
    pub checkers: Arena<CheckerDef>,
    pub covergroups: Arena<CovergroupDef>,
    pub coverpoints: Arena<CoverpointDef>,
    pub crosses: Arena<CrossDef>,
    pub subroutine: Option<Subroutine>,
    pub package_imports: Arena<PackageImport>,
    pub param_ports: Option<IdxRange<Declarator>>,
    pub ports: Ports,
    pub cont_assigns: Arena<ContAssign>,
    pub defparams: Arena<DefParam>,
    pub generate_regions: Arena<GenerateRegion>,
    pub generate_kind: GenerateBlockKind,
    pub specify_blocks: Arena<SpecifyBlock>,
    pub specify_items: Arena<SpecifyItem>,
    pub modports: Arena<ModportDef>,
    pub default_clocking: Option<DefaultClockingRef>,
    pub clocking_blocks: Arena<ClockingBlockDef>,
    pub instantiations: Arena<Instantiation>,
    pub inst_param_assigns: Arena<ParamAssign>,
    pub instances: Arena<Instance>,
    pub inst_port_conns: Arena<PortConn>,
}

impl Body {
    pub fn scope(&self, owner: OwnerId) -> Option<&BodyScopeData> {
        self.scope_graph.scope(owner)
    }

    pub fn module_owners(&self) -> impl Iterator<Item = OwnerId> + '_ {
        self.items.iter().filter_map(|item| match item {
            BodyItem::ModuleOwner(owner) => Some(*owner),
            _ => None,
        })
    }

    pub fn subroutine_owners(&self) -> impl Iterator<Item = OwnerId> + '_ {
        self.items.iter().filter_map(|item| match item {
            BodyItem::SubroutineOwner(owner) => Some(*owner),
            _ => None,
        })
    }

    pub fn declaration(&self, id: DeclarationId) -> &Declaration {
        &self.declarations[id]
    }

    pub fn typedef(&self, id: TypedefId) -> &Typedef {
        &self.typedefs[id]
    }

    pub fn struct_def(&self, id: StructId) -> &StructDef {
        &self.structs[id]
    }

    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id]
    }

    pub fn event_expr(&self, id: crate::expr::timing_control::EventExprId) -> &EventExpr {
        &self.event_exprs[id]
    }

    pub fn declarator(&self, id: DeclId) -> &Declarator {
        &self.decls[id]
    }

    pub fn stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id]
    }

    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
        self.scope_graph.shrink_to_fit();
        self.declarations.shrink_to_fit();
        self.typedefs.shrink_to_fit();
        self.structs.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.stmts.shrink_to_fit();
        self.procs.shrink_to_fit();
        self.config_decls.shrink_to_fit();
        self.udp_decls.shrink_to_fit();
        self.library_decls.shrink_to_fit();
        self.library_includes.shrink_to_fit();
        self.checkers.shrink_to_fit();
        self.covergroups.shrink_to_fit();
        self.coverpoints.shrink_to_fit();
        self.crosses.shrink_to_fit();
        self.package_imports.shrink_to_fit();
        self.ports.shrink_to_fit();
        self.cont_assigns.shrink_to_fit();
        self.defparams.shrink_to_fit();
        self.generate_regions.shrink_to_fit();
        self.specify_blocks.shrink_to_fit();
        self.specify_items.shrink_to_fit();
        self.modports.shrink_to_fit();
        self.clocking_blocks.shrink_to_fit();
        self.instantiations.shrink_to_fit();
        self.inst_param_assigns.shrink_to_fit();
        self.instances.shrink_to_fit();
        self.inst_port_conns.shrink_to_fit();
    }
}

/// Source identities for all HIR arenas in an owner-local [`Body`]. Current
/// pointers and ranges remain exclusively in `AstIdMap`/`SourceProjection`.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct BodySourceMap {
    pub declaration_srcs: SourceMap<Declaration>,
    pub typedef_srcs: SourceMap<Typedef>,
    pub struct_srcs: SourceMap<StructDef>,
    pub expr_srcs: SourceMap<Expr>,
    pub event_expr_srcs: SourceMap<EventExpr>,
    pub decl_srcs: SourceMap<Declarator>,
    pub stmt_srcs: SourceMap<Stmt>,
    pub proc_srcs: SourceMap<Proc>,
    pub config_decl_srcs: SourceMap<ConfigDecl>,
    pub udp_decl_srcs: SourceMap<UdpDecl>,
    pub library_decl_srcs: SourceMap<LibraryDecl>,
    pub library_include_srcs: SourceMap<LibraryInclude>,
    pub checker_srcs: SourceMap<CheckerDef>,
    pub covergroup_srcs: SourceMap<CovergroupDef>,
    pub coverpoint_srcs: SourceMap<CoverpointDef>,
    pub cross_srcs: SourceMap<CrossDef>,
    pub port_srcs: PortSrcs,
    pub assign_srcs: SourceMap<ContAssign>,
    pub defparam_srcs: SourceMap<DefParam>,
    pub generate_region_srcs: SourceMap<GenerateRegion>,
    pub specify_block_srcs: SourceMap<SpecifyBlock>,
    pub specify_item_srcs: SourceMap<SpecifyItem>,
    pub modport_srcs: SourceMap<ModportDef>,
    pub default_clocking_src: Option<SourceAstId>,
    pub clocking_block_srcs: SourceMap<ClockingBlockDef>,
    pub instantiation_srcs: SourceMap<Instantiation>,
    pub inst_param_assign_srcs: SourceMap<ParamAssign>,
    pub instance_srcs: SourceMap<Instance>,
    pub inst_port_conn_srcs: SourceMap<PortConn>,
}

impl BodySourceMap {
    pub fn expr_from_source(&self, source: crate::ast_id_map::SourceAstId) -> Option<ExprId> {
        self.expr_srcs.src_to_hir(source)
    }

    pub fn shrink_to_fit(&mut self) {
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
        self.proc_srcs.shrink_to_fit();
        self.config_decl_srcs.shrink_to_fit();
        self.udp_decl_srcs.shrink_to_fit();
        self.library_decl_srcs.shrink_to_fit();
        self.library_include_srcs.shrink_to_fit();
        self.checker_srcs.shrink_to_fit();
        self.covergroup_srcs.shrink_to_fit();
        self.coverpoint_srcs.shrink_to_fit();
        self.cross_srcs.shrink_to_fit();
        self.port_srcs.shrink_to_fit();
        self.assign_srcs.shrink_to_fit();
        self.defparam_srcs.shrink_to_fit();
        self.generate_region_srcs.shrink_to_fit();
        self.specify_block_srcs.shrink_to_fit();
        self.specify_item_srcs.shrink_to_fit();
        self.modport_srcs.shrink_to_fit();
        self.clocking_block_srcs.shrink_to_fit();
        self.instantiation_srcs.shrink_to_fit();
        self.inst_param_assign_srcs.shrink_to_fit();
        self.instance_srcs.shrink_to_fit();
        self.inst_port_conn_srcs.shrink_to_fit();
    }
}
impl LoweredData for Body {
    type SourceMap = BodySourceMap;
}

#[salsa::tracked(lru = 512, returns(clone))]
fn body_input(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    match owner.kind(db) {
        OwnerKind::ProceduralBlock => lower_procedural_body(db, owner),
        OwnerKind::Subroutine => lower_subroutine_body(db, owner),
        OwnerKind::Block => body_with_source_map(db, body_owner(db, owner)),
        OwnerKind::File => {
            crate::file::lower_file_owner(db, owner, &LoweringSyntax::for_owner(db, owner))
        }
        OwnerKind::Module => {
            crate::module::lower_module_owner(db, owner, &LoweringSyntax::for_owner(db, owner))
        }
        OwnerKind::GenerateBlock => crate::module::generate::lower_generate_owner(
            db,
            owner,
            &LoweringSyntax::for_owner(db, owner),
        ),
        OwnerKind::Checker => {
            crate::checker::lower_checker_owner(db, owner, &LoweringSyntax::for_owner(db, owner))
        }
        OwnerKind::Covergroup => crate::covergroup::lower_covergroup_owner(
            db,
            owner,
            &LoweringSyntax::for_owner(db, owner),
        ),
        OwnerKind::ClockingBlock => crate::module::clocking::lower_clocking_owner(
            db,
            owner,
            &LoweringSyntax::for_owner(db, owner),
        ),
    }
}
#[salsa::tracked(lru = 512, returns(clone))]
pub(crate) fn body_with_source_map(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    body_input(db, owner)
}

pub(crate) fn body_owner(db: &dyn HirDefDb, mut owner: OwnerId) -> OwnerId {
    while owner.kind(db) == OwnerKind::Block {
        owner = owner.parent(db).expect("block scope must have a body owner ancestor");
    }
    owner
}

fn lower_procedural_body(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(proc) =
        db.ast_id_map(file_id).node(owner.ast_id(db), &tree).and_then(ast::ProceduralBlock::cast)
    else {
        return Arc::new(Lowered::new(file_id, Body::default(), BodySourceMap::default()));
    };

    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut ctx =
        LoweringCtx::new(db, owner, BodyStore { data: &mut body, sources: &mut source_map });
    let root_stmt = ctx.record_stmt(proc.statement());
    let diagnostics = ctx.emit_diagnostics();
    drop(ctx);
    body.root_stmt = Some(root_stmt);
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}

fn lower_subroutine_body(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(func) = owner_node(db, owner, &tree).and_then(ast::FunctionDeclaration::cast) else {
        return empty_body(file_id);
    };
    if func.end().is_none() {
        return empty_body(file_id);
    }

    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut ctx =
        LoweringCtx::new(db, owner, BodyStore { data: &mut body, sources: &mut source_map });
    let ast_ids = Arc::clone(&ctx.ast_ids);
    let tree = ctx.tree.clone();
    let Some(subroutine) = lower_subroutine(&func, |ty| ctx.lower_data_ty(ty), &ast_ids, &tree)
    else {
        return empty_body(file_id);
    };
    ctx.store.body().0.subroutine = Some(subroutine);
    ctx.lower_subroutine_items(func);
    let diagnostics = ctx.emit_diagnostics();
    drop(ctx);
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new_with_diagnostics(file_id, body, source_map, diagnostics))
}

fn owner_node<'tree>(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &'tree syntax::SyntaxTree,
) -> Option<syntax::SyntaxNode<'tree>> {
    db.ast_id_map(owner.file(db)).node(owner.ast_id(db), tree)
}

fn empty_body(file_id: preproc_expand::file::HirFileId) -> Arc<Lowered<Body>> {
    Arc::new(Lowered::new(file_id, Body::default(), BodySourceMap::default()))
}

impl<Store: crate::lower::LoweringStore> LoweringCtx<Store> {
    pub(crate) fn lower_body_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container = self.current_owner();
        let struct_def = lower_struct_def(struct_ty, container, |ty| self.lower_data_ty(ty));
        let source = self.source_id(struct_ty.syntax());
        let (body, sources) = self.store.body();
        crate::alloc_with_source_entry(
            &mut body.structs,
            &mut sources.struct_srcs,
            struct_def,
            source,
        )
    }

    fn lower_body_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let source = self.source_id(typedef.syntax());
        let typedef_id = {
            let (body, sources) = self.store.body();
            crate::alloc_with_source_entry(
                &mut body.typedefs,
                &mut sources.typedef_srcs,
                Typedef { name: lower_ident_opt(typedef.name()), ty: None },
                source,
            )
        };
        self.record_body_typedef(typedef_id);
        let container = self.current_owner();
        let ty = lower_typedef_data_ty(
            self,
            typedef.type_(),
            container,
            |ctx, struct_ty| ctx.lower_body_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );
        self.store.body().0.typedefs[typedef_id].ty = Some(ty);
        typedef_id
    }

    fn lower_body_local_variable_decl(
        &mut self,
        local_decl: ast::LocalVariableDeclaration,
    ) -> DeclarationId {
        let ty = self.lower_data_ty(local_decl.type_());
        let parent = self.alloc_declaration(
            DataDecl {
                ty,
                const_kw: false,
                var_kw: local_decl.var().is_some(),
                decls: empty_decls_range(),
            },
            local_decl,
        );
        let decls = self.lower_declarators(local_decl.declarators(), parent.into());
        self.finish_declaration_decls(parent, decls);
        parent
    }

    pub(crate) fn lower_nested_block(&mut self, block: ast::BlockStatement, owner: OwnerId) {
        self.enter_body_scope(owner);
        for node in block.items().children() {
            let item = match_ast! { node.syntax(),
                ast::Statement[it] => Some(BlockItem::StmtId(self.lower_stmt(it))),
                ast::DataDeclaration[it] => { self.lower_data_decl(it); None },
                ast::ParameterDeclarationStatement[it] => {
                    self.lower_param_decl_base(it.parameter());
                    None
                },
                ast::TypedefDeclaration[it] => { self.lower_body_typedef(it); None },
                _ => {
                    self.report_unsupported(node.syntax(), "unsupported nested-block item");
                    continue;
                },
            };
            if let Some(item) = item {
                self.push_body_item(item);
            }
        }
        self.leave_body_scope(owner);
    }
}

impl LoweringCtx<BodyStore<'_>> {
    fn record_stmt(&mut self, stmt: ast::Statement) -> StmtId {
        self.lower_stmt(stmt)
    }

    fn lower_subroutine_items(&mut self, func: ast::FunctionDeclaration) {
        for item in func.items().children() {
            let syntax = item.syntax();
            let body_item = match_ast! { syntax,
                ast::Statement[it] => Some(BlockItem::StmtId(self.record_stmt(it))),
                ast::DataDeclaration[it] => { self.lower_data_decl(it); None },
                ast::PortDeclaration[it] => {
                    self.lower_port_decl_as_data_decl(it);
                    None
                },
                ast::LocalVariableDeclaration[it] => {
                    self.lower_body_local_variable_decl(it);
                    None
                },
                ast::ParameterDeclarationStatement[it] => {
                    self.lower_param_decl_base(it.parameter());
                    None
                },
                ast::TypedefDeclaration[it] => { self.lower_body_typedef(it); None },
                _ => {
                    self.report_unsupported(syntax, "unsupported subroutine item");
                    continue;
                },
            };
            if let Some(body_item) = body_item {
                self.push_body_item(body_item);
            }
        }
    }
}

impl BodySourceMap {
    pub fn item_to_source(
        &self,
        db: &dyn HirDefDb,
        item: &BodyItem,
    ) -> Option<crate::ast_id_map::SourceAstId> {
        match item {
            BodyItem::ModuleOwner(owner)
            | BodyItem::GenerateBlockOwner(owner)
            | BodyItem::SubroutineOwner(owner)
            | BodyItem::CheckerOwner(owner)
            | BodyItem::CovergroupOwner(owner)
            | BodyItem::ClockingBlockOwner(owner) => Some(owner.ast_id(db)),
            BodyItem::ProcId(id) => self.proc_srcs.hir_to_src(*id),
            BodyItem::DeclarationId(id) => self.declaration_srcs.hir_to_src(*id),
            BodyItem::TypedefId(id) => self.typedef_srcs.hir_to_src(*id),
            BodyItem::StructId(id) => self.struct_srcs.hir_to_src(*id),
            BodyItem::ConfigDeclId(id) => self.config_decl_srcs.hir_to_src(*id),
            BodyItem::UdpDeclId(id) => self.udp_decl_srcs.hir_to_src(*id),
            BodyItem::LibraryDeclId(id) => self.library_decl_srcs.hir_to_src(*id),
            BodyItem::LibraryIncludeId(id) => self.library_include_srcs.hir_to_src(*id),
            BodyItem::ContAssignId(id) => self.assign_srcs.hir_to_src(*id),
            BodyItem::DefParamId(id) => self.defparam_srcs.hir_to_src(*id),
            BodyItem::GenerateRegionId(id) => self.generate_region_srcs.hir_to_src(*id),
            BodyItem::SpecifyBlockId(id) => self.specify_block_srcs.hir_to_src(*id),
            BodyItem::SpecifyItemId(id) => self.specify_item_srcs.hir_to_src(*id),
            BodyItem::InstantiationId(id) => self.instantiation_srcs.hir_to_src(*id),
            BodyItem::PortDeclId(id) => utils::get::Get::get(&self.port_srcs, *id),
            BodyItem::ModportId(id) => self.modport_srcs.hir_to_src(*id),
        }
    }

    pub fn block_item_to_source(&self, item: &BlockItem) -> Option<crate::ast_id_map::SourceAstId> {
        match item {
            BlockItem::DeclarationId(id) => self.declaration_srcs.hir_to_src(*id),
            BlockItem::TypedefId(id) => self.typedef_srcs.hir_to_src(*id),
            BlockItem::StructId(id) => self.struct_srcs.hir_to_src(*id),
            BlockItem::StmtId(id) => self.stmt_srcs.hir_to_src(*id),
        }
    }
}

pub(crate) fn set_body_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    body_input::set_lru_capacity(db, capacity);
    body_with_source_map::set_lru_capacity(db, capacity);
}

crate::impl_arena_getters!(
    Body;
    DeclarationId => declarations => Declaration,
    TypedefId => typedefs => Typedef,
    StructId => structs => StructDef,
    ExprId => exprs => Expr,
    EventExprId => event_exprs => EventExpr,
    DeclId => decls => Declarator,
    StmtId => stmts => Stmt,
    ProcId => procs => Proc,
    ConfigDeclId => config_decls => ConfigDecl,
    UdpDeclId => udp_decls => UdpDecl,
    LibraryDeclId => library_decls => LibraryDecl,
    LibraryIncludeId => library_includes => LibraryInclude,
    CheckerId => checkers => CheckerDef,
    CovergroupId => covergroups => CovergroupDef,
    CoverpointId => coverpoints => CoverpointDef,
    CrossId => crosses => CrossDef,
    Idx<PackageImport> => package_imports => PackageImport,
    ContAssignId => cont_assigns => ContAssign,
    DefParamId => defparams => DefParam,
    GenerateRegionId => generate_regions => GenerateRegion,
    SpecifyBlockId => specify_blocks => SpecifyBlock,
    SpecifyItemId => specify_items => SpecifyItem,
    ModportId => modports => ModportDef,
    ClockingBlockId => clocking_blocks => ClockingBlockDef,
    InstantiationId => instantiations => Instantiation,
    ParamAssignId => inst_param_assigns => ParamAssign,
    InstanceId => instances => Instance,
    PortConnId => inst_port_conns => PortConn,
    NonAnsiPortId => ports => NonAnsiPort,
    PortRefId => ports => PortRef,
    PortDeclId => ports => PortDecl,
);

crate::impl_source_map_getters!(
    BodySourceMap;
    DeclarationId => declaration_srcs,
    TypedefId => typedef_srcs,
    StructId => struct_srcs,
    ExprId => expr_srcs,
    EventExprId => event_expr_srcs,
    DeclId => decl_srcs,
    StmtId => stmt_srcs,
    ProcId => proc_srcs,
    ConfigDeclId => config_decl_srcs,
    UdpDeclId => udp_decl_srcs,
    LibraryDeclId => library_decl_srcs,
    LibraryIncludeId => library_include_srcs,
    CheckerId => checker_srcs,
    CovergroupId => covergroup_srcs,
    CoverpointId => coverpoint_srcs,
    CrossId => cross_srcs,
    ContAssignId => assign_srcs,
    DefParamId => defparam_srcs,
    GenerateRegionId => generate_region_srcs,
    SpecifyBlockId => specify_block_srcs,
    SpecifyItemId => specify_item_srcs,
    ModportId => modport_srcs,
    ClockingBlockId => clocking_block_srcs,
    InstantiationId => instantiation_srcs,
    ParamAssignId => inst_param_assign_srcs,
    InstanceId => instance_srcs,
    PortConnId => inst_port_conn_srcs,
    NonAnsiPortId => port_srcs,
    PortRefId => port_srcs,
    PortDeclId => port_srcs,
);
