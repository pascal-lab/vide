use base_db::intern::Lookup;
use la_arena::Arena;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
    match_ast,
};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use super::{
    Ident,
    declaration::{Declaration, DeclarationId, DeclarationSrc, LowerDeclaration},
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    stmt::{LowerStmt, Stmt, StmtId, StmtKind, StmtSrc},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    define_src,
    file::HirFileId,
    hir_def::lower_ident_opt,
    impl_arena_idx, impl_lower_decl, impl_lower_declaration, impl_lower_event_expr,
    impl_lower_expr, impl_lower_stmt, impl_source_map_idx,
    source_map::{SourceMap, ToAstNode},
};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct Block {
    pub name: Option<Ident>,
    pub kind: BlockKind,
    pub items: SmallVec<[BlockItem; 2]>,

    pub declarations: Arena<Declaration>,
    pub stmts: Arena<Stmt>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
}

#[derive(Default, Debug, PartialEq, Eq, Clone, Hash)]
pub enum BlockKind {
    #[default]
    Sequential,
    Parallel(ParBlockKind),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ParBlockKind {
    Join,
    JoinAny,
    JoinNone,
}

define_src!(BlockSrc(ast::BlockStatement));

impl From<BlockSrc> for StmtSrc {
    fn from(src: BlockSrc) -> Self {
        StmtSrc::new(src.0)
    }
}

impl Get<LocalBlockId> for SourceMap<StmtSrc, Stmt> {
    type Output = BlockSrc;

    fn get_opt(&self, block_id: LocalBlockId) -> Option<Self::Output> {
        let stmt_id = block_id.0;
        Some(BlockSrc(self.get(stmt_id).ptr()))
    }
}

impl Get<BlockSrc> for SourceMap<StmtSrc, Stmt> {
    type Output = LocalBlockId;

    fn get_opt(&self, block_src: BlockSrc) -> Option<Self::Output> {
        let src: StmtSrc = block_src.into();
        Some(LocalBlockId(self.get(src)))
    }
}

impl GetRef<LocalBlockId> for Arena<Stmt> {
    type Output = BlockInfo;

    fn get_opt(&self, block_id: LocalBlockId) -> Option<&Self::Output> {
        let stmt_id = block_id.0;
        let Stmt { kind: StmtKind::Block(block_info), .. } = &self[stmt_id] else {
            unreachable!();
        };
        Some(block_info)
    }
}

impl_arena_idx! { Block =>
    declarations[Declaration],
    stmts[Stmt],
    stmts[LocalBlockId => BlockInfo],
    exprs[Expr],
    event_exprs[EventExpr],
    decls[Declarator],
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum BlockItem {
        DeclarationId,
        StmtId,
    }
}

impl Block {
    pub fn shrink_to_fit(&mut self) {
        self.declarations.shrink_to_fit();
        self.stmts.shrink_to_fit();
        self.exprs.shrink_to_fit();
        self.event_exprs.shrink_to_fit();
        self.decls.shrink_to_fit();
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct BlockInfo {
    pub name: Option<Ident>,
    pub block_id: BlockId,
}

pub struct LocalBlockId(StmtId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BlockId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct BlockLoc {
    pub cont_id: ContainerId,
    pub src: InFile<BlockSrc>,
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct BlockSourceMap {
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
}

impl_source_map_idx! { BlockSourceMap =>
    declaration_srcs[DeclarationSrc, DeclarationId],
    stmt_srcs[StmtSrc, StmtId],
    stmt_srcs[BlockSrc, LocalBlockId],
    expr_srcs[ExprSrc, ExprId],
    event_expr_srcs[EventExprSrc, EventExprId],
    decl_srcs[DeclaratorSrc, DeclId],
}

impl BlockSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.declaration_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
    }
}

pub(crate) struct LowerBlockCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) block_id: BlockId,

    pub(crate) block: &'a mut Block,
    pub(crate) block_source_map: &'a mut BlockSourceMap,
}

impl_lower_expr!(LowerBlockCtx<'_>, block, block_source_map);
impl_lower_decl!(LowerBlockCtx<'_>, block, block_source_map);
impl_lower_event_expr!(LowerBlockCtx<'_>, block, block_source_map);
impl_lower_stmt!(LowerBlockCtx<'_>, block_id, block, block_source_map);
impl_lower_declaration!(LowerBlockCtx<'_>, block, block_source_map);

impl LowerBlockCtx<'_> {
    pub(crate) fn lower_block(&mut self, block: ast::BlockStatement) {
        // TODO: label? end_block_name?
        self.block.name = block.block_name().and_then(|name| lower_ident_opt(name.name()));
        self.block.kind = match block.end().map(|end| end.kind()) {
            Some(TokenKind::JOIN_KEYWORD) => BlockKind::Parallel(ParBlockKind::Join),
            Some(TokenKind::JOIN_ANY_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinAny),
            Some(TokenKind::JOIN_NONE_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinNone),
            _ => BlockKind::Sequential, // Some(TokenKind::END_KEYWORD) | None | Others
        };
        self.block.items = block
            .items()
            .children()
            .map(|node| {
                let node = node.syntax();
                match_ast! { node in
                    ast::Statement as it => self.stmt_ctx().lower_stmt(it).into(),
                    ast::DataDeclaration as it => self.declaration_ctx().lower_data_decl(it).into(),
                    _ => unimplemented!("{:?}", node.kind()),
                }
            })
            .collect();
    }
}

pub(crate) fn block_with_source_map_query(
    db: &dyn HirDb,
    block_id: BlockId,
) -> (Arc<Block>, Arc<BlockSourceMap>) {
    let InFile { cont_id: file_id, value: block_src } = block_id.lookup(db).src;
    let tree = db.parse(file_id);

    let mut block = Block::default();
    let mut block_source_map = BlockSourceMap::default();
    let Some(ast_block) = block_src.to_node(&tree) else {
        return (Arc::new(block), Arc::new(block_source_map));
    };

    let mut lower_ctx = LowerBlockCtx {
        db,
        file_id,
        block_id,
        block: &mut block,
        block_source_map: &mut block_source_map,
    };
    lower_ctx.lower_block(ast_block);

    block.shrink_to_fit();
    block_source_map.shrink_to_fit();
    (Arc::new(block), Arc::new(block_source_map))
}
