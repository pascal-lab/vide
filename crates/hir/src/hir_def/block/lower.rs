use la_arena::Arena;
use syntax::ast::{self, ptr::EventExpressionPtr};
use utils::try_;

use crate::{
    container::InFile,
    db::InternDb,
    file::HirFileId,
    hir_def::{
        control::{EventExpr, LowerDelayControl, LowerEventExpr, LowerTimingControl},
        data::{
            DataDecl, DataDeclSrc, LowerDataDecl, LowerDataType, LowerDelay, LowerDimension,
            LowerSubDecl, SubDecl, SubDeclSrc,
        },
        expr::{Expr, ExprSrc, LowerExpr},
        literal::LowerLiteral,
        lower::Lower,
        stmt::{LowerStmt, Stmt, StmtSrc},
        SourceMap,
    },
    try_match,
};

use super::{
    block_src::BlockSrc, Block, BlockId, BlockInfo, BlockKind, BlockSourceMap, JoinKeyword,
};

pub fn lower_join_keyword(keyword: &ast::JoinKeyword) -> Option<JoinKeyword> {
    try_match! {
        keyword.token_join(), _ => Some(JoinKeyword::Join),
        keyword.token_join_any(), _ => Some(JoinKeyword::JoinAny),
        keyword.token_join_none(), _ => Some(JoinKeyword::JoinNone),
        _ => None,
    }
}

pub(crate) struct BlockLowerCtx<'a> {
    pub db: &'a dyn InternDb,
    pub block: &'a mut Block,
    pub block_src_map: &'a mut BlockSourceMap,
    pub file_id: HirFileId,
    pub file_text: &'a str,
}

impl Lower for BlockLowerCtx<'_> {
    type ContainerId = BlockId;

    fn db(&self) -> &dyn InternDb {
        self.db
    }

    fn container_id(&self) -> BlockId {
        self.block.info.block_id
    }

    fn file_id(&self) -> HirFileId {
        self.file_id
    }

    fn file_text(&self) -> &str {
        self.file_text
    }
}

impl LowerLiteral for BlockLowerCtx<'_> {}

impl LowerExpr for BlockLowerCtx<'_> {
    fn arena_expr(&mut self) -> &mut Arena<Expr> {
        &mut self.block.data.exprs
    }

    fn src_map_expr(&mut self) -> &mut SourceMap<ExprSrc, Expr> {
        &mut self.block_src_map.expr
    }
}

impl LowerDataType for BlockLowerCtx<'_> {}

impl LowerDimension for BlockLowerCtx<'_> {}

impl LowerTimingControl for BlockLowerCtx<'_> {}

impl LowerDelayControl for BlockLowerCtx<'_> {}

impl LowerEventExpr for BlockLowerCtx<'_> {
    fn arena_event_exprs(&mut self) -> &mut Arena<EventExpr> {
        &mut self.block.data.event_exprs
    }

    fn src_map_event_expr(&mut self) -> &mut SourceMap<InFile<EventExpressionPtr>, EventExpr> {
        &mut self.block_src_map.event_exprs
    }
}

impl LowerStmt for BlockLowerCtx<'_> {
    fn arena_stmts(&mut self) -> &mut Arena<Stmt> {
        &mut self.block.data.stmts
    }

    fn arena_blocks(&mut self) -> &mut Arena<BlockInfo> {
        &mut self.block.data.block_infos
    }

    fn src_map_stmt(&mut self) -> &mut SourceMap<StmtSrc, Stmt> {
        &mut self.block_src_map.stmt
    }

    fn src_map_blocks(&mut self) -> &mut SourceMap<BlockSrc, BlockInfo> {
        &mut self.block_src_map.block
    }
}

impl LowerDataDecl for BlockLowerCtx<'_> {
    fn arena_data_decl(&mut self) -> &mut Arena<DataDecl> {
        &mut self.block.data.data_decls
    }

    fn src_map_data_decl(&mut self) -> &mut SourceMap<DataDeclSrc, DataDecl> {
        &mut self.block_src_map.data_decl
    }
}

impl LowerDelay for BlockLowerCtx<'_> {}

impl LowerSubDecl for BlockLowerCtx<'_> {
    fn arena_sub_decl(&mut self) -> &mut Arena<SubDecl> {
        &mut self.block.data.sub_decls
    }

    fn src_map_sub_decl(&mut self) -> &mut SourceMap<SubDeclSrc, SubDecl> {
        &mut self.block_src_map.sub_decl
    }
}

impl<'a> BlockLowerCtx<'a> {
    pub(crate) fn lower_seq_block(&mut self, seq_block: &ast::SeqBlock) {
        self.block.info.ident =
            seq_block.identifiers().next().and_then(|ident| self.lower_ident(&ident));
        self.block.kind = BlockKind::Sequential;

        for item in seq_block.block_item_declarations() {
            if let Some(block_item) = self.lower_block_item_decl(&item) {
                self.block.data.block_item_decls.push(block_item);
            }
        }

        for stmt in seq_block.statement_or_nulls() {
            self.lower_stmt_or_null(&stmt);
        }
    }

    pub(crate) fn lower_par_block(&mut self, par_block: &ast::ParBlock) {
        self.block.info.ident =
            par_block.identifiers().next().and_then(|ident| self.lower_ident(&ident));

        let Some(kind) = try_!(lower_join_keyword(&par_block.join_keyword()?)?) else {
            return;
        };
        self.block.kind = BlockKind::Parallel(kind);

        for item in par_block.block_item_declarations() {
            if let Some(block_item) = self.lower_block_item_decl(&item) {
                self.block.data.block_item_decls.push(block_item);
            }
        }

        for stmt in par_block.statement_or_nulls() {
            self.lower_stmt_or_null(&stmt);
        }
    }
}
