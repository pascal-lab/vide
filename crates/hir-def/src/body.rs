use la_arena::Arena;
use smallvec::SmallVec;
use syntax::ast::{self, AstNode};
use triomphe::Arc;

use crate::{
    aggregate::{StructDef, StructSrc},
    container::ArenaOwnerId,
    db::HirDefDb,
    declaration::{Declaration, DeclarationSrc},
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprSrc},
    },
    lower::{BodyStore, LoweringCtx},
    owner::{OwnerId, OwnerKind},
    source_map::{DiagnosticSource, Lowered, LoweredData, LoweringDiagnostic, SourceMap},
    stmt::{Stmt, StmtId, StmtSrc},
    typedef::{Typedef, TypedefSrc},
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
        OwnerKind::Subroutine => {
            let lowered = crate::subroutine::subroutine_body_with_source_map(db, owner);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        OwnerKind::Block => {
            let lowered = crate::block::block_with_source_map(db, owner);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        OwnerKind::GenerateBlock => {
            let lowered = crate::module::generate::generate_block_with_source_map(db, owner);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        OwnerKind::Module => {
            let lowered = crate::module::module_with_source_map(db, owner);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
        }
        OwnerKind::File => {
            let file = db.syntax_file(owner.file(db));
            let lowered = crate::file::hir_file_with_source_map(db, file);
            cloned_body(&lowered.body, &lowered.source_map().body, lowered.diagnostics())
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

fn cloned_body(
    body: &Body,
    source_map: &BodySourceMap,
    diagnostics: &[LoweringDiagnostic],
) -> Arc<Lowered<Body>> {
    let mut source_map = source_map.clone();
    source_map.diagnostics = diagnostics.to_vec();
    Arc::new(Lowered::new(body.clone(), source_map))
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
);
