use base_db::intern::Lookup;
use la_arena::Arena;
use proc_macro_utils::define_container;
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
    alloc_idx_and_src,
    declaration::{
        Declaration, DeclarationId, DeclarationSrc, LowerDeclaration, impl_lower_declaration,
    },
    expr::{
        Expr, ExprSrc, LowerExpr,
        declarator::{Declarator, DeclaratorSrc, impl_lower_decl},
        impl_lower_expr,
        timing_control::{EventExpr, EventExprSrc, impl_lower_event_expr},
    },
    lower_ident_opt,
    stmt::{LowerStmt, Stmt, StmtId, StmtKind, StmtSrc, impl_lower_stmt},
    typedef::{Typedef, TypedefId, TypedefSrc, lower_typedef_data_ty},
};
use crate::{
    container::{ContainerId, InFile},
    db::{HirDb, InternDb},
    define_src_with_name,
    file::HirFileId,
    region_tree::{RegionTree, RegionTreeBuilder},
    source_map::{SourceMap, ToAstNode},
};

define_container! {
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct Block {
        name: Option<Ident>,
        kind: BlockKind,

        declarations: [Declaration],
        typedefs: [Typedef],
        structs: [StructDef],
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
    #[derive(Default, Debug, PartialEq, Eq)]
    pub struct BlockSourceMap {
        items: SmallVec<[BlockItem; 2]>,
        region_tree: RegionTree,

        declaration_srcs: [Declaration | DeclarationSrc],
        typedef_srcs: [Typedef | TypedefSrc],
        struct_srcs: [StructDef | StructSrc],
        expr_srcs: [Expr | ExprSrc],
        event_expr_srcs: [EventExpr | EventExprSrc],
        decl_srcs: [Declarator | DeclaratorSrc],
        stmt_srcs: [Stmt | StmtSrc] => {
            [StmtId | StmtSrc],
            [LocalBlockId | BlockSrc],
        },
        block_srcs: FxHashMap<BlockSrc, LocalBlockId>,
    }
}

impl BlockSourceMap {
    pub fn item_to_ptr(&self, item: &BlockItem) -> SyntaxNodePtr {
        match item {
            BlockItem::DeclarationId(idx) => self.get(*idx).ptr(),
            BlockItem::TypedefId(idx) => self.get(*idx).ptr(),
            BlockItem::StructId(idx) => self.get(*idx).node,
            BlockItem::StmtId(idx) => self.get(*idx).node,
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

impl TryFrom<StmtSrc> for BlockSrc {
    type Error = ();

    fn try_from(StmtSrc { node, name }: StmtSrc) -> Result<Self, Self::Error> {
        if !ast::BlockStatement::can_cast(node.kind()) {
            return Err(());
        }

        Ok(BlockSrc { node, name })
    }
}

impl Get<LocalBlockId> for SourceMap<StmtSrc, Stmt> {
    type Output = BlockSrc;

    fn get(&self, block_id: LocalBlockId) -> Self::Output {
        let stmt_id = block_id.0;
        self.get(stmt_id).try_into().unwrap()
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
    pub cont_id: ContainerId,
    pub src: InFile<BlockSrc>,
}

pub(crate) struct LowerBlockCtx<'a> {
    pub(crate) db: &'a dyn InternDb,
    pub(crate) file_id: HirFileId,
    pub(crate) block_id: BlockId,

    pub(crate) block: &'a mut Block,
    pub(crate) block_source_map: &'a mut BlockSourceMap,

    pub(crate) region_tree: RegionTreeBuilder,
}

impl_lower_expr!(LowerBlockCtx<'_>, block, block_source_map);
impl_lower_decl!(LowerBlockCtx<'_>, block, block_source_map);
impl_lower_event_expr!(LowerBlockCtx<'_>, block, block_source_map);
impl_lower_stmt!(LowerBlockCtx<'_>, block_id, block, block_source_map);
impl_lower_declaration!(LowerBlockCtx<'_>, block, block_source_map);

impl LowerBlockCtx<'_> {
    fn lower_struct_type(&mut self, struct_ty: ast::StructUnionType) -> StructId {
        let container_id = ContainerId::BlockId(self.block_id);
        let struct_def = lower_struct_def(struct_ty.clone(), container_id, |ty| {
            self.expr_ctx().lower_data_ty(ty)
        });

        alloc_idx_and_src! {
            struct_def => self.block.structs,
            struct_ty => self.block_source_map.struct_srcs,
        }
    }

    fn lower_typedef(&mut self, typedef: ast::TypedefDeclaration) -> TypedefId {
        let name = lower_ident_opt(typedef.name());

        let typedef_id = alloc_idx_and_src! {
            Typedef { name, ty: None } => self.block.typedefs,
            typedef => self.block_source_map.typedef_srcs,
        };

        let data_ty = typedef.type_();
        let lowered_ty = lower_typedef_data_ty(
            self,
            data_ty,
            ContainerId::BlockId(self.block_id),
            |ctx, struct_ty| ctx.lower_struct_type(struct_ty),
            |ctx, ty| ctx.expr_ctx().lower_data_ty(ty),
        );

        self.block.typedefs[typedef_id].ty = Some(lowered_ty);

        typedef_id
    }

    pub(crate) fn lower_block(&mut self, block: ast::BlockStatement) {
        // TODO: label? end_block_name?
        self.block.name = block.block_name().and_then(|name| lower_ident_opt(name.name()));
        self.block.kind = match block.end().map(|end| end.kind()) {
            Some(TokenKind::JOIN_KEYWORD) => BlockKind::Parallel(ParBlockKind::Join),
            Some(TokenKind::JOIN_ANY_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinAny),
            Some(TokenKind::JOIN_NONE_KEYWORD) => BlockKind::Parallel(ParBlockKind::JoinNone),
            _ => BlockKind::Sequential, // Some(TokenKind::END_KEYWORD) | None | Others
        };

        for node in block.items().children() {
            let idx = match_ast! { node.syntax(),
                ast::Statement[it] => {
                    let stmt_id = self.stmt_ctx().lower_stmt(it);
                    if let Some(block_stmt) = it.as_block_statement() {
                        let block_src = BlockSrc::from(block_stmt);
                        let local_block_id = LocalBlockId(stmt_id);
                        self.block_source_map.block_srcs.insert(block_src, local_block_id);
                    }
                    stmt_id.into()
                },
                ast::DataDeclaration[it] => self.declaration_ctx().lower_data_decl(it).into(),
                ast::TypedefDeclaration[it] => self.lower_typedef(it).into(),
                _ => unimplemented!("{:?}", node.syntax().kind()),
            };
            self.block_source_map.items.push(idx);
            self.region_tree.handle_node(node.syntax());
        }

        self.region_tree.stage(block.end());
        self.block_source_map.region_tree = self.region_tree.finish();
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
        region_tree: RegionTreeBuilder::new(),
    };
    lower_ctx.lower_block(ast_block);

    block.shrink_to_fit();
    block_source_map.shrink_to_fit();
    (Arc::new(block), Arc::new(block_source_map))
}
