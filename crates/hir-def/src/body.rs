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
    block::{BlockItem, BlockSrc, LocalBlockId},
    container::ArenaOwnerId,
    db::HirDefDb,
    declaration::{DataDecl, Declaration, DeclarationId, DeclarationSrc},
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc, empty_decls_range},
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

/// The owner-local semantic body.
///
/// A body owns the arenas shared by files, modules, generate blocks,
/// procedural blocks, and subroutines. Owner-specific headers and members are
/// kept outside this structure; this is the only allocation store for
/// expressions, statements, declarations, and local type definitions.
#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Body {
    pub items: SmallVec<[crate::block::BlockItem; 2]>,
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
    pub fn shrink_to_fit(&mut self) {
        self.declarations.shrink_to_fit();
        self.items.shrink_to_fit();
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
    pub block_srcs: FxHashMap<BlockSrc, LocalBlockId>,
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
        self.block_srcs.shrink_to_fit();
        self.diagnostics.shrink_to_fit();
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

#[salsa::tracked(lru = 512, returns(clone))]
pub(crate) fn body_with_source_map(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    match owner.kind(db) {
        OwnerKind::ProceduralBlock => lower_procedural_body(db, owner),
        OwnerKind::Subroutine => lower_subroutine_body(db, owner),
        OwnerKind::Block => lower_block_body(db, owner),
        OwnerKind::GenerateBlock | OwnerKind::Module | OwnerKind::File => {
            lower_legacy_owner_body(db, owner)
        }
        OwnerKind::Checker | OwnerKind::Covergroup | OwnerKind::ClockingBlock => {
            panic!("scope-only owner has no body: {:?}", owner.kind(db))
        }
    }
}

fn lower_procedural_body(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(proc) = db
        .owner_source_ast_id(owner)
        .and_then(|ast_id| db.ast_id_map(file_id).ptr(ast_id))
        .and_then(|ptr| ptr.to_node(&tree))
        .and_then(ast::ProceduralBlock::cast)
    else {
        return Arc::new(Lowered::new(Body::default(), BodySourceMap::default()));
    };

    let mut body = Body::default();
    let mut source_map = BodySourceMap::default();
    let mut ctx = LoweringCtx::new(
        db,
        file_id,
        ArenaOwnerId::Owner(owner),
        BodyStore { data: &mut body, sources: &mut source_map },
    );
    let root_stmt = ctx.lower_stmt(proc.statement());
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

fn lower_block_body(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(block) = owner_node(db, owner, &tree).and_then(ast::BlockStatement::cast) else {
        return empty_body();
    };

    lower_body(db, owner, |ctx| ctx.lower_block_items(block))
}

fn lower_legacy_owner_body(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Lowered<Body>> {
    match owner.kind(db) {
        OwnerKind::GenerateBlock => {
            let lowered = crate::module::generate::generate_block_with_source_map(db, owner);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        OwnerKind::Module => {
            let lowered = crate::module::module_with_source_map(db, owner);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        OwnerKind::File => {
            let lowered = crate::file::hir_file_with_source_map(db, db.syntax_file(owner.file(db)));
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        kind => unreachable!("{kind:?} is not a legacy body owner"),
    }
}

fn owner_node<'tree>(
    db: &dyn HirDefDb,
    owner: OwnerId,
    tree: &'tree syntax::SyntaxTree,
) -> Option<syntax::SyntaxNode<'tree>> {
    db.owner_source_ast_id(owner)
        .and_then(|ast_id| db.ast_id_map(owner.file(db)).ptr(ast_id))
        .and_then(|ptr| ptr.to_node(tree))
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
        owner.file(db),
        ArenaOwnerId::Owner(owner),
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

impl LoweringCtx<BodyStore<'_>> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let struct_def =
            lower_struct_def(struct_ty, self.owner.clone(), |ty| self.lower_data_ty(ty));
        alloc_with_source(
            self.file_id,
            &mut self.store.data.structs,
            &mut self.store.sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let typedef_id = alloc_with_source(
            self.file_id,
            &mut self.store.data.typedefs,
            &mut self.store.sources.typedef_srcs,
            Typedef { name: lower_ident_opt(typedef.name()), ty: None },
            typedef,
        );
        let ty = lower_typedef_data_ty(
            self,
            typedef.type_(),
            self.owner.clone(),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );
        self.store.data.typedefs[typedef_id].ty = Some(ty);
        typedef_id
    }

    fn lower_local_variable_decl(
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

    fn record_stmt(&mut self, stmt: ast::Statement) -> StmtId {
        let stmt_id = self.lower_stmt(stmt);
        if let Some(block) = stmt.as_block_statement() {
            self.store
                .sources
                .block_srcs
                .insert(BlockSrc::from_ast(self.file_id, block), LocalBlockId(stmt_id));
        }
        stmt_id
    }

    fn lower_block_items(&mut self, block: ast::BlockStatement) {
        for node in block.items().children() {
            let item = match_ast! { node.syntax(),
                ast::Statement[it] => BlockItem::StmtId(self.record_stmt(it)),
                ast::DataDeclaration[it] => BlockItem::DeclarationId(self.lower_data_decl(it)),
                ast::ParameterDeclarationStatement[it] => {
                    BlockItem::DeclarationId(self.lower_param_decl_base(it.parameter()))
                },
                ast::TypedefDeclaration[it] => BlockItem::TypedefId(self.lower_typedef(it)),
                _ => continue,
            };
            self.store.data.items.push(item);
            self.region_tree.handle_node(node.syntax());
        }
        self.store.sources.region_tree = self.region_tree.finish();
    }

    fn lower_subroutine_items(&mut self, func: ast::FunctionDeclaration) {
        for item in func.items().children() {
            self.region_tree.handle_node(item.syntax());
            let syntax = item.syntax();
            match_ast! { syntax,
                ast::Statement[it] => {
                    let stmt_id = self.record_stmt(it);
                    self.store.data.items.push(BlockItem::StmtId(stmt_id));
                },
                ast::DataDeclaration[it] => {
                    let id = self.lower_data_decl(it);
                    self.store.data.items.push(BlockItem::DeclarationId(id));
                },
                ast::PortDeclaration[it] => {
                    if let Some(id) = self.lower_port_decl_as_data_decl(it) {
                        self.store.data.items.push(BlockItem::DeclarationId(id));
                    }
                },
                ast::LocalVariableDeclaration[it] => {
                    let id = self.lower_local_variable_decl(it);
                    self.store.data.items.push(BlockItem::DeclarationId(id));
                },
                ast::ParameterDeclarationStatement[it] => {
                    let id = self.lower_param_decl_base(it.parameter());
                    self.store.data.items.push(BlockItem::DeclarationId(id));
                },
                ast::TypedefDeclaration[it] => {
                    let id = self.lower_typedef(it);
                    self.store.data.items.push(BlockItem::TypedefId(id));
                },
                _ => {},
            }
        }
        self.region_tree.stage(func.end(), func.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
    }
}

fn cloned_body(
    body: &Body,
    source_map: &BodySourceMap,
    diagnostics: &[LoweringDiagnostic],
) -> Arc<Lowered<Body>> {
    let mut source_map = source_map.clone();
    source_map.diagnostics = diagnostics.to_vec();
    Arc::new(Lowered::new(body.clone(), source_map))
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
    crate::block::LocalBlockId => stmts => crate::block::BlockInfo,
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
    crate::block::BlockSrc => crate::block::LocalBlockId => stmt_srcs,
);
