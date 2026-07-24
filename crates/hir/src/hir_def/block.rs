use la_arena::Arena;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use syntax::{
    TokenKind,
    ast::{self, AstNode},
    match_ast,
    ptr::SyntaxNodePtr,
};
use triomphe::Arc;
use utils::{
    define_enum_deriving_from,
    get::{Get, GetRef},
};

use super::{
    Ident,
    aggregate::{StructDef, StructId, StructSrc, lower_struct_def},
    alloc_with_source,
    declaration::{Declaration, DeclarationId, DeclarationSrc},
    expr::{
        Expr, ExprId, ExprSrc,
        declarator::{DeclId, Declarator, DeclaratorSrc},
        timing_control::{EventExpr, EventExprId, EventExprSrc},
    },
    lower::{BlockStore, LoweringCtx},
    lower_ident_opt,
    stmt::{Stmt, StmtId, StmtKind, StmtSrc},
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    base_db::intern::Lookup,
    container::{ArenaOwnerId, InFile},
    db::HirDb,
    region_tree::RegionTree,
    source_map::{AstKind, IsNamedSrc, IsSrc, NamedAstId, SourceMap, ToAstNode},
};

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: Option<Ident>,
    pub kind: BlockKind,
    pub declarations: Arena<Declaration>,
    pub typedefs: Arena<Typedef>,
    pub structs: Arena<StructDef>,
    pub exprs: Arena<Expr>,
    pub event_exprs: Arena<EventExpr>,
    pub decls: Arena<Declarator>,
    pub stmts: Arena<Stmt>,
}

impl Block {
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

#[derive(Default, Debug, PartialEq, Eq)]
pub struct BlockSourceMap {
    pub items: SmallVec<[BlockItem; 2]>,
    pub region_tree: RegionTree,
    pub declaration_srcs: SourceMap<DeclarationSrc, Declaration>,
    pub typedef_srcs: SourceMap<TypedefSrc, Typedef>,
    pub struct_srcs: SourceMap<StructSrc, StructDef>,
    pub expr_srcs: SourceMap<ExprSrc, Expr>,
    pub event_expr_srcs: SourceMap<EventExprSrc, EventExpr>,
    pub decl_srcs: SourceMap<DeclaratorSrc, Declarator>,
    pub stmt_srcs: SourceMap<StmtSrc, Stmt>,
    pub block_srcs: FxHashMap<BlockSrc, LocalBlockId>,
}

impl BlockSourceMap {
    pub fn shrink_to_fit(&mut self) {
        self.declaration_srcs.shrink_to_fit();
        self.typedef_srcs.shrink_to_fit();
        self.struct_srcs.shrink_to_fit();
        self.expr_srcs.shrink_to_fit();
        self.event_expr_srcs.shrink_to_fit();
        self.decl_srcs.shrink_to_fit();
        self.stmt_srcs.shrink_to_fit();
    }
}

crate::hir_def::impl_arena_getters!(
    Block;
    DeclarationId => declarations => Declaration,
    TypedefId => typedefs => Typedef,
    StructId => structs => StructDef,
    ExprId => exprs => Expr,
    EventExprId => event_exprs => EventExpr,
    DeclId => decls => Declarator,
    StmtId => stmts => Stmt,
    LocalBlockId => stmts => BlockInfo,
);

crate::hir_def::impl_source_map_getters!(
    BlockSourceMap;
    DeclarationSrc => DeclarationId => declaration_srcs,
    TypedefSrc => TypedefId => typedef_srcs,
    StructSrc => StructId => struct_srcs,
    ExprSrc => ExprId => expr_srcs,
    EventExprSrc => EventExprId => event_expr_srcs,
    DeclaratorSrc => DeclId => decl_srcs,
    StmtSrc => StmtId => stmt_srcs,
    BlockSrc => LocalBlockId => stmt_srcs,
);

impl BlockSourceMap {
    pub fn item_to_ptr(&self, item: &BlockItem) -> Option<SyntaxNodePtr> {
        Some(match item {
            BlockItem::DeclarationId(idx) => self.get(*idx)?.ptr(),
            BlockItem::TypedefId(idx) => self.get(*idx)?.ptr(),
            BlockItem::StructId(idx) => self.get(*idx)?.node,
            BlockItem::StmtId(idx) => self.get(*idx)?.node,
        })
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
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

pub(crate) fn find_local_block_id(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BlockId(pub salsa::InternId);

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct BlockLoc {
    pub cont_id: ArenaOwnerId,
    pub src: InFile<BlockSrc>,
}

pub(crate) type LowerBlockCtx<'a> = LoweringCtx<'a, BlockStore<'a>>;

impl LowerBlockCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ArenaOwnerId::Block(self.block_id());
        let struct_def = lower_struct_def(struct_ty, container_id, |ty| self.lower_data_ty(ty));

        alloc_with_source(
            self.file_id,
            &mut self.store.data.structs,
            &mut self.store.sources.struct_srcs,
            struct_def,
            struct_ty,
        )
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());

        let typedef_id = alloc_with_source(
            self.file_id,
            &mut self.store.data.typedefs,
            &mut self.store.sources.typedef_srcs,
            Typedef { name, ty: None },
            typedef,
        );

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            ArenaOwnerId::Block(self.block_id()),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.lower_data_ty(ty),
        );

        self.store.data.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    pub(crate) fn lower_block(&mut self, block: ast::BlockStatement) {
        // TODO: label? end_block_name?
        self.store.data.name = block.block_name().and_then(|name| lower_ident_opt(name.name()));
        self.store.data.kind = match block.end().map(|end| end.kind()) {
            Some(TokenKind::JOIN_KEYWORD) => BlockKind::Parallel(ParBlockKind::Join),
            Some(TokenKind::JOIN_ANY_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinAny),
            Some(TokenKind::JOIN_NONE_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinNone),
            _ => BlockKind::Sequential, // Some(TokenKind::END_KEYWORD) | None | Others
        };

        for node in block.items().children() {
            let idx = match_ast! { node.syntax(),
                ast::Statement[it] => {
                    let stmt_id = self.lower_stmt(it);
                    if let Some(block_stmt) = it.as_block_statement() {
                        let block_src = BlockSrc::from_ast(self.file_id, block_stmt);
                        let local_block_id = LocalBlockId(stmt_id);
                        self.store.sources.block_srcs.insert(block_src, local_block_id);
                    }
                    stmt_id.into()
                },
                ast::DataDeclaration[it] => self.lower_data_decl(it).into(),
                ast::ParameterDeclarationStatement[it] => {
                    self.lower_param_decl_base(it.parameter()).into()
                },
                ast::TypedefDeclaration[it] => self.lower_typedef(it).into(),
                _ => continue,
            };
            self.store.sources.items.push(idx);
            self.region_tree.handle_node(node.syntax());
        }

        self.region_tree.stage(block.end(), block.syntax());
        self.store.sources.region_tree = self.region_tree.finish();
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

    let mut lower_ctx = LoweringCtx::new(
        db,
        file_id,
        block_id.into(),
        BlockStore { data: &mut block, sources: &mut block_source_map },
    );
    lower_ctx.lower_block(ast_block);
    lower_ctx.emit_diagnostics();

    block.shrink_to_fit();
    block_source_map.shrink_to_fit();
    (Arc::new(block), Arc::new(block_source_map))
}
