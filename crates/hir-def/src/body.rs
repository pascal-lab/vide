use la_arena::Arena;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    ast::{self, AstNode},
    match_ast,
    ptr::SyntaxNodePtr,
};
use triomphe::Arc;

use crate::{
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_with_source,
    block::BlockItem,
    db::HirDefDb,
    declaration::{DataDecl, Declaration, DeclarationId, DeclarationSrc},
    expr::{
        Expr, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc, empty_decls_range},
        timing_control::{EventExpr, EventExprSrc},
    },
    lower::{BodyStore, LoweringCtx},
    lower_ident_opt,
    owner::{OwnerId, OwnerKind},
    region_tree::RegionTree,
    source_map::{DiagnosticSource, Lowered, LoweredData, LoweringDiagnostic, SourceMap},
    stmt::{Stmt, StmtId, StmtSrc},
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
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

/// The owner-local semantic body. Nested blocks share these arenas and are
/// partitioned by [`BodyScopeGraph`] instead of opening child body queries.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Body {
    pub scope_graph: BodyScopeGraph,
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
    pub root_stmt: Option<StmtId>,
}

impl Body {
    pub fn scope(&self, owner: OwnerId) -> Option<&BodyScopeData> {
        self.scope_graph.scope(owner)
    }

    pub fn shrink_to_fit(&mut self) {
        self.scope_graph.shrink_to_fit();
        self.declarations.shrink_to_fit();
        self.typedefs.shrink_to_fit();
        self.structs.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
        self.stmts.shrink_to_fit();
    }
}

/// Source projection for the common owner-local body arenas.
///
/// Position data is deliberately kept out of [`Body`], so source edits can
/// invalidate projection without changing semantic body data.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct BodySourceMap {
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub typedef_srcs: SourceMap<TypedefSrc, Typedef>,
    pub struct_srcs: SourceMap<StructSrc, StructDef>,
    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
    pub region_tree: RegionTree,
    scope_region_trees: FxHashMap<OwnerId, RegionTree>,
    pub diagnostics: Vec<LoweringDiagnostic>,
}

impl BodySourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
        self.diagnostics.shrink_to_fit();
        self.scope_region_trees.shrink_to_fit();
    }

    pub fn region_tree_for(&self, body: &Body, owner: OwnerId) -> Option<&RegionTree> {
        if body.scope_graph.root() == Some(owner) {
            Some(&self.region_tree)
        } else {
            self.scope_region_trees.get(&owner)
        }
    }

    pub(crate) fn insert_scope_region_tree(&mut self, owner: OwnerId, regions: RegionTree) {
        let previous = self.scope_region_trees.insert(owner, regions);
        assert!(previous.is_none(), "body scope region tree inserted twice");
    }
}
impl LoweredData for Body {
    type SourceMap = BodySourceMap;
}

impl DiagnosticSource for BodySourceMap {
    fn diagnostics(&self) -> &[LoweringDiagnostic] {
        &self.diagnostics
    }
}

/// Internal result of lowering a structural owner. The aggregate guarantees
/// one lowering pass; tracked projection queries publish structure and body
/// independently so Salsa can backdate either value.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct OwnerLowering<T: LoweredData> {
    pub(crate) structure: Arc<Lowered<T>>,
    pub(crate) body: Arc<Lowered<Body>>,
}

impl<T: LoweredData> OwnerLowering<T> {
    pub(crate) fn new(
        structure: T,
        structure_sources: T::SourceMap,
        body: Body,
        body_sources: BodySourceMap,
    ) -> Self {
        Self {
            structure: Arc::new(Lowered::new(structure, structure_sources)),
            body: Arc::new(Lowered::new(body, body_sources)),
        }
    }
}

#[salsa::tracked(lru = 512, returns(clone))]
pub(crate) fn body_with_source_map(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    match owner.kind(db) {
        OwnerKind::ProceduralBlock => lower_procedural_body(db, owner),
        OwnerKind::Subroutine => lower_subroutine_body(db, owner),
        OwnerKind::Block => body_with_source_map(db, body_owner(db, owner)),
        OwnerKind::GenerateBlock => {
            crate::module::generate::generate_block_body_with_source_map(db, owner)
        }
        OwnerKind::Module => crate::module::module_body_with_source_map(db, owner),
        OwnerKind::File => {
            crate::file::file_body_with_source_map(db, db.syntax_file(owner.file(db)))
        }
        OwnerKind::Checker | OwnerKind::Covergroup | OwnerKind::ClockingBlock => {
            panic!("scope-only owner has no body: {:?}", owner.kind(db))
        }
    }
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
    let Some(proc) = db
        .owner_source_ast_id(owner)
        .and_then(|ast_id| db.ast_id_map(file_id).node(ast_id, &tree))
        .and_then(ast::ProceduralBlock::cast)
    else {
        return Arc::new(Lowered::new(Body::default(), BodySourceMap::default()));
    };

    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut ctx = LoweringCtx::new(
        db,
        owner,
        BodyStore { data: &mut body, sources: &mut source_map },
    );
    let root_stmt = ctx.record_stmt(proc.statement());
    let diagnostics = ctx.emit_diagnostics();
    drop(ctx);
    body.root_stmt = Some(root_stmt);
    source_map.diagnostics = diagnostics;
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new(body, source_map))
}

fn lower_subroutine_body(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(func) = owner_node(db, owner, &tree).and_then(ast::FunctionDeclaration::cast) else {
        return empty_body();
    };
    if func.end().is_none() {
        return empty_body();
    }

    lower_body(db, owner, |ctx| ctx.lower_subroutine_items(func))
}


fn owner_node<'tree>(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &'tree syntax::SyntaxTree,
) -> Option<syntax::SyntaxNode<'tree>> {
    db.owner_source_ast_id(owner)
        .and_then(|ast_id| db.ast_id_map(owner.file(db)).node(ast_id, tree))
}

fn lower_body(
    db: &dyn HirDefDb,
    owner: OwnerId,
    lower: impl FnOnce(&mut LoweringCtx<BodyStore<'_>>),
) -> Arc<Lowered<Body>> {
    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut ctx = LoweringCtx::new(
        db,
        owner,
        BodyStore { data: &mut body, sources: &mut source_map },
    );
    lower(&mut ctx);
    let diagnostics = ctx.emit_diagnostics();
    drop(ctx);
    source_map.diagnostics = diagnostics;
    body.shrink_to_fit();
    source_map.shrink_to_fit();
    Arc::new(Lowered::new(body, source_map))
}

fn empty_body() -> Arc<Lowered<Body>> {
    Arc::new(Lowered::new(Body::default(), BodySourceMap::default()))
}

impl<Store: crate::lower::LoweringStore> LoweringCtx<Store> {
    fn lower_body_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container = self.current_arena_owner();
        let struct_def = lower_struct_def(struct_ty, container, |ty| self.lower_data_ty(ty));
        let file_id = self.file_id;
        let (body, sources) = self.store.body();
        alloc_with_source(
            file_id,
            &mut body.structs,
            &mut sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_body_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let file_id = self.file_id;
        let typedef_id = {
            let (body, sources) = self.store.body();
            alloc_with_source(
                file_id,
                &mut body.typedefs,
                &mut sources.typedef_srcs,
                Typedef { name: lower_ident_opt(typedef.name()), ty: None },
                typedef,
            )
        };
        self.record_body_typedef(typedef_id);
        let container = self.current_arena_owner();
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
        let mut regions = crate::region_tree::RegionTreeBuilder::new();
        for node in block.items().children() {
            let item = match_ast! { node.syntax(),
                ast::Statement[it] => Some(BlockItem::StmtId(self.lower_stmt(it))),
                ast::DataDeclaration[it] => { self.lower_data_decl(it); None },
                ast::ParameterDeclarationStatement[it] => {
                    self.lower_param_decl_base(it.parameter());
                    None
                },
                ast::TypedefDeclaration[it] => { self.lower_body_typedef(it); None },
                _ => continue,
            };
            if let Some(item) = item {
                self.push_body_item(item);
            }
            regions.handle_node(node.syntax());
        }
        self.set_scope_region_tree(owner, regions.finish());
        self.leave_body_scope(owner);
    }
}

impl LoweringCtx<BodyStore<'_>> {
    fn record_stmt(&mut self, stmt: ast::Statement) -> StmtId {
        self.lower_stmt(stmt)
    }

    fn lower_subroutine_items(&mut self, func: ast::FunctionDeclaration) {
        for item in func.items().children() {
            self.region_tree.handle_node(item.syntax());
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
                _ => continue,
            };
            if let Some(body_item) = body_item {
                self.push_body_item(body_item);
            }
        }
        self.region_tree.stage(func.end(), func.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

impl BodySourceMap {
    pub fn item_to_ptr(&self, item: &BlockItem) -> Option<SyntaxNodePtr> {
        Some(match item {
            BlockItem::DeclarationId(id) => self.declaration_srcs.hir_to_src(*id)?.ptr(),
            BlockItem::TypedefId(id) => self.typedef_srcs.hir_to_src(*id)?.ptr(),
            BlockItem::StructId(id) => self.struct_srcs.hir_to_src(*id)?.node,
            BlockItem::StmtId(id) => self.stmt_srcs.hir_to_src(*id)?.node,
        })
    }
}

pub(crate) fn set_body_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    body_with_source_map::set_lru_capacity(db, capacity);
}

crate::impl_arena_getters!(
    Body;
    crate::declaration::DeclarationId => declarations => Declaration,
    crate::typedef::TypedefId => typedefs => Typedef,
    crate::aggregate::StructId => structs => StructDef,
    crate::expr::ExprId => exprs => Expr,
    crate::expr::timing_control::EventExprId => event_exprs => EventExpr,
    crate::expr::declarator::DeclId => decls => Declarator,
    crate::stmt::StmtId => stmts => Stmt,
);

crate::impl_source_map_getters!(
    BodySourceMap;
    crate::declaration::DeclarationSrc => crate::declaration::DeclarationId => declaration_srcs,
    crate::typedef::TypedefSrc => crate::typedef::TypedefId => typedef_srcs,
    crate::aggregate::StructSrc => crate::aggregate::StructId => struct_srcs,
    crate::expr::ExprSrc => crate::expr::ExprId => expr_srcs,
    crate::expr::timing_control::EventExprSrc => crate::expr::timing_control::EventExprId => event_expr_srcs,
    crate::expr::declarator::DeclaratorSrc => crate::expr::declarator::DeclId => decl_srcs,
    crate::stmt::StmtSrc => crate::stmt::StmtId => stmt_srcs,
);
