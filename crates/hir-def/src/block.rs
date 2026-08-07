use la_arena::Arena;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use super::{
    Ident,
    stmt::{Stmt, StmtId, StmtKind, StmtSrc},
};
use crate::{
    aggregate::StructId,
    container::{ArenaOwnerId, InFile},
    db::HirDefDb,
    declaration::DeclarationId,
    owner::{OwnerId, OwnerKind},
    source_map::{AstKind, IsNamedSrc, IsSrc, NamedAstId, SourceMap},
    typedef::TypedefId,
};

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: Option<Ident>,
    pub kind: BlockKind,
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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct BlockStatementAst;

impl AstKind for BlockStatementAst {
    type Node<'a> = ast::BlockStatement<'a>;
}

pub type BlockSrc = NamedAstId<BlockStatementAst>;

impl From<BlockSrc> for StmtSrc {
    fn from(src: BlockSrc) -> Self {
        StmtSrc::new(src.file_id, src.node, src.name)
    }
}

impl TryFrom<StmtSrc> for BlockSrc {
    type Error = ();

    fn try_from(src: StmtSrc) -> Result<Self, Self::Error> {
        let node = src.node;
        if !ast::BlockStatement::can_cast(node.kind()) {
            return Err(());
        }

        Ok(BlockSrc::new(src.file_id, node, src.name))
    }
}

impl Get<LocalBlockId> for SourceMap<StmtSrc, Stmt> {
    type Output = Option<BlockSrc>;

    fn get(&self, block_id: LocalBlockId) -> Self::Output {
        let stmt_id = block_id.0;
        self.hir_to_src(stmt_id)?.try_into().ok()
    }
}

impl Get<BlockSrc> for SourceMap<StmtSrc, Stmt> {
    type Output = Option<LocalBlockId>;

    fn get(&self, block_src: BlockSrc) -> Self::Output {
        find_local_block_id(self, block_src)
    }
}

pub fn find_local_block_id(
    stmt_srcs: &SourceMap<StmtSrc, Stmt>,
    block_src: BlockSrc,
) -> Option<LocalBlockId> {
    let src: StmtSrc = block_src.into();
    if let Some((stmt_id, _)) = stmt_srcs.iter().find(|(_, stmt_src)| **stmt_src == src) {
        return Some(LocalBlockId(stmt_id));
    }

    let block_kind = block_src.kind();
    let block_range = block_src.range();
    let block_name_range = block_src.name_range();
    let (stmt_id, _) = stmt_srcs
        .iter()
        .find(|(_, stmt_src)| {
            stmt_src.kind() == block_kind
                && stmt_src.range() == block_range
                && stmt_src.name_range() == block_name_range
        })
        .or_else(|| {
            stmt_srcs.iter().find(|(_, stmt_src)| {
                stmt_src.kind() == block_kind && stmt_src.range() == block_range
            })
        })?;
    Some(LocalBlockId(stmt_id))
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
        TypedefId,
        StructId,
        StmtId,
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct BlockInfo {
    pub name: Option<Ident>,
    pub block_id: BlockId,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct LocalBlockId(pub StmtId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockId(Arc<BlockLoc>);

impl BlockId {
    pub fn new(loc: BlockLoc) -> Self {
        Self(Arc::new(loc))
    }

    pub fn loc(&self) -> &BlockLoc {
        &self.0
    }
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct BlockLoc {
    pub cont_id: ArenaOwnerId,
    pub src: InFile<BlockSrc>,
}

#[salsa::tracked(lru = 128, returns(clone))]
pub(crate) fn block_data(db: &dyn HirDefDb, owner: OwnerId) -> Arc<Block> {
    debug_assert_eq!(owner.kind(db), OwnerKind::Block);
    let file_id = owner.file(db);
    let tree = db.parse(file_id);
    let Some(block) = db
        .owner_source_ast_id(owner)
        .and_then(|ast_id| db.ast_id_map(file_id).ptr(ast_id))
        .and_then(|ptr| ptr.to_node(&tree))
        .and_then(ast::BlockStatement::cast)
    else {
        return Arc::new(Block::default());
    };

    let name = block.block_name().and_then(|name| crate::lower_ident_opt(name.name()));
    let kind = match block.end().map(|end| end.kind()) {
        Some(TokenKind::JOIN_KEYWORD) => BlockKind::Parallel(ParBlockKind::Join),
        Some(TokenKind::JOIN_ANY_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinAny),
        Some(TokenKind::JOIN_NONE_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinNone),
        _ => BlockKind::Sequential,
    };
    Arc::new(Block { name, kind })
}

pub(crate) fn set_block_lru_capacity(db: &mut dyn HirDefDb, capacity: usize) {
    block_data::set_lru_capacity(db, capacity);
}
