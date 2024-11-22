use base_db::intern::Lookup;
use la_arena::Arena;
use proc_macro_utils::define_container;
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
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc,
        declarator::{Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc, impl_lower_event_expr},
    },
    lower_ident_opt,
    stmt::{LowerStmt, Stmt, StmtId, StmtKind, StmtSrc, impl_lower_stmt},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    define_src_with_name,
    file::HirFileId,
    source_map::{SourceMap, ToAstNode},
};

define_container! {
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct Block {
        name: Option<Ident>,
        kind: BlockKind,
        items: SmallVec<[BlockItem; 2]>,

        declarations: [Declaration],
        exprs: [Expr],
        event_exprs: [EventExpr],
        decls: [Declarator],
        stmts: [Stmt] => {
            [StmtId | Stmt],
            [LocalBlockId | BlockInfo],
        }
    }
}

define_container! {
    #[derive(Default, Debug, PartialEq, Eq, Clone)]
    pub struct BlockSourceMap {
        declaration_srcs: [Declaration | DeclarationSrc],
        expr_srcs: [Expr | ExprSrc],
        event_expr_srcs: [EventExpr | EventExprSrc],
        decl_srcs: [Declarator | DeclaratorSrc],
        stmt_srcs: [Stmt | StmtSrc] => {
            [StmtId | StmtSrc],
            [LocalBlockId | BlockSrc],
        }
    }
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

define_src_with_name!(BlockSrc(ast::BlockStatement));

impl From<BlockSrc> for StmtSrc {
    fn from(BlockSrc { node, name }: BlockSrc) -> Self {
        StmtSrc { node, name }
    }
}

impl From<StmtSrc> for BlockSrc {
    fn from(StmtSrc { node, name }: StmtSrc) -> Self {
        assert!(ast::BlockStatement::can_cast(node.kind()));
        BlockSrc { node, name }
    }
}

impl Get<LocalBlockId> for SourceMap<StmtSrc, Stmt> {
    type Output = BlockSrc;

    fn get(&self, block_id: LocalBlockId) -> Self::Output {
        let stmt_id = block_id.0;
        self.get(stmt_id).into()
    }
}

impl Get<BlockSrc> for SourceMap<StmtSrc, Stmt> {
    type Output = LocalBlockId;

    fn get(&self, block_src: BlockSrc) -> Self::Output {
        let src: StmtSrc = block_src.into();
        LocalBlockId(self.get(src))
    }
}

impl GetRef<LocalBlockId> for Arena<Stmt> {
    type Output = BlockInfo;

    fn get(&self, block_id: LocalBlockId) -> &Self::Output {
        let stmt_id = block_id.0;
        let Stmt { kind: StmtKind::Block(block_info), .. } = &self[stmt_id] else {
            unreachable!();
        };
        block_info
    }
}

define_enum_deriving_from! {
    #[derive(Debug, PartialEq, Eq, Clone)]
    pub enum BlockItem {
        DeclarationId,
        StmtId,
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
                match_ast! { node.syntax(),
                    ast::Statement[it] => self.stmt_ctx().lower_stmt(it).into(),
                    ast::DataDeclaration[it] => self.declaration_ctx().lower_data_decl(it).into(),
                    _ => unimplemented!("{:?}", node.syntax().kind()),
                }
            })
            .collect();
    }
}

pub(crate) fn block_with_source_map_query(
    db: &dyn HirDb,
    block_id: BlockId,
) -> (Arc<Block>, Arc<BlockSourceMap>) {
    let InFile { file_id, value: block_src } = block_id.lookup(db).src;
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
