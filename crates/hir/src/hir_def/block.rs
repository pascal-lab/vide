use crate::{file::InFile, hir_def::{data::DataDecl, stmt::StmtId, try_match, Ident}};
use la_arena::Idx;
use smallvec::SmallVec;
use syntax::ast::{self, ptr};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Block {
    pub kind: BlockKind,
    pub ident: Option<Ident>,
    pub item_decls: SmallVec<[BlockItemDecl; 1]>,
    pub stmts: SmallVec<[StmtId; 1]>,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum LocalBlockSrc {
    SeqBlock(ptr::SeqBlockPtr),
    ParBlock(ptr::ParBlockPtr),
}

pub type BlockSrc = InFile<LocalBlockSrc>;

pub type LocalBlockId = Idx<Block>;

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

pub(crate) fn lower_join_keyword(keyword: &ast::JoinKeyword) -> Option<JoinKeyword> {
    try_match! {
        keyword.token_join(), _ => Some(JoinKeyword::Join),
        keyword.token_join_any(), _ => Some(JoinKeyword::JoinAny),
        keyword.token_join_none(), _ => Some(JoinKeyword::JoinNone),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BlockItemDecl {
    DataDecl(Idx<DataDecl>),
    // TODO: LetDecl(),
}
