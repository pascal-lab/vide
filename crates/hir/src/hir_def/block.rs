pub mod block_src;
pub mod lower;

use crate::{
    container::ContainerId,
    db::HirDb,
    file::InFile,
    hir_def::{block::block_src::LocalBlockSrc, data::DataDecl, Ident},
    impl_index,
};
use base_db::intern::Lookup;
use la_arena::{Arena, Idx, IdxRange};
use salsa;
use smallvec::SmallVec;
use std::ops::Index;
use syntax::ast::ptr;
use triomphe::Arc;
use utils::try_;

use self::block_src::BlockSrc;

use super::{
    control::EventExpr,
    data::{DataDeclSrc, DataSubDecl, DataSubDeclSrc},
    expr::{Expr, ExprSrc},
    stmt::{Stmt, StmtSrc},
    SourceMap,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Block {
    pub info: BlockInfo,
    pub kind: BlockKind,
    pub data: BlockData,
}

impl_index! (Block for
    Expr, data,
    DataSubDecl, data,
    DataDecl, data,
    BlockInfo, data,
);

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct BlockData {
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub stmts: Arena<Stmt>,
    pub data_decls: Arena<DataDecl>,
    pub data_sub_decls: Arena<DataSubDecl>,
    pub block_infos: Arena<BlockInfo>,
    pub block_item_decls: SmallVec<[BlockItemDecl; 1]>,
}

impl_index! (BlockData for
    Expr, exprs,
    EventExpr, event_exprs,
    Stmt, stmts,
    DataSubDecl, data_sub_decls,
    DataDecl, data_decls,
    BlockInfo, block_infos,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BlockId(pub salsa::InternId);
#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct BlockLoc {
    pub container: ContainerId,
    pub block_src: BlockSrc,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BlockInfo {
    pub ident: Option<Ident>,
    pub block_id: BlockId,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum BlockKind {
    Sequential,
    Parallel(JoinKeyword),
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum JoinKeyword {
    Join,
    JoinAny,
    JoinNone,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BlockItemDecl {
    DataDecl(Idx<DataDecl>),
    // TODO: LetDecl(),
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct BlockSourceMap {
    pub expr: SourceMap<ExprSrc, Expr>,
    pub event_exprs: SourceMap<InFile<ptr::EventExpressionPtr>, EventExpr>,
    pub data_sub_decl: SourceMap<DataSubDeclSrc, DataSubDecl>,
    pub data_decl: SourceMap<DataDeclSrc, DataDecl>,
    pub stmt: SourceMap<StmtSrc, Stmt>,
    pub block: SourceMap<BlockSrc, BlockInfo>,
}

pub(crate) fn block_with_source_map_query(
    db: &dyn HirDb,
    block_id: BlockId,
) -> (Arc<Block>, Arc<BlockSourceMap>) {
    let BlockLoc { block_src, .. } = block_id.lookup(db);
    let InFile { file_id, value: block_ptr } = block_src;

    let mut block = Block {
        info: BlockInfo { ident: None, block_id },
        kind: BlockKind::Sequential,
        data: BlockData::default(),
    };
    let mut block_src_map = BlockSourceMap::default();

    try_! {
        let syntax_tree = db.hir_syntax_tree(file_id)?;
        let file_text = &db.hir_file_text(file_id);

        let mut ctx = lower::BlockLowerCtx {
            db,
            block: &mut block,
            block_src_map: &mut block_src_map,
            file_id,
            file_text: file_text.as_ref(),
        };

        match block_ptr {
            LocalBlockSrc::SeqBlock(block_ptr) => {
                let node = block_ptr.to_node(syntax_tree.tree())?;
                ctx.lower_seq_block(&node);
            }
            LocalBlockSrc::ParBlock(block_ptr) => {
                let node = block_ptr.to_node(syntax_tree.tree())?;
                ctx.lower_par_block(&node);
            }
        }
    };

    (Arc::new(block), Arc::new(block_src_map))
}
