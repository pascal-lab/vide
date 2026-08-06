use la_arena::Arena;

use crate::{
    aggregate::{StructDef, StructSrc},
    declaration::{Declaration, DeclarationSrc},
    expr::{Expr, ExprSrc, timing_control::{EventExpr, EventExprSrc}},
    expr::declarator::{Declarator, DeclaratorSrc},
    source_map::{LoweringDiagnostic, SourceMap},
    stmt::{Stmt, StmtSrc},
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
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
}

impl Body {
    pub fn shrink_to_fit(&mut self) {
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
