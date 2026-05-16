use std::{cell::RefCell, ops};

use hir_to_def::Hir2DefCache;
use itertools::{Either, Itertools};
use pathres::PathResolution;
use rustc_hash::FxHashMap;
use source_to_def::{Source2DefCache, Source2DefCtx};
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt,
    ast::{self, AstNode},
};
use utils::text_edit::TextSize;
use vfs::FileId;

use crate::{
    container::{ContainerId, InContainer, InFile},
    db::HirDb,
    file::HirFileId,
    hir_def::{
        Ident,
        block::{BlockId, BlockSrc},
        expr::ExprId,
        subroutine::{SubroutineId, SubroutineSrc},
    },
};

mod hir_to_def;
pub mod pathres;
pub mod resolver;
mod source_to_def;

pub struct Semantics<'db, DB> {
    pub db: &'db DB,
    impl_: SemanticsImpl<'db>,
}

impl<DB: HirDb> Semantics<'_, DB> {
    pub fn new(db: &DB) -> Semantics<'_, DB> {
        let impl_ = SemanticsImpl::new(db);
        Semantics { db, impl_ }
    }
}

impl<'db, DB> ops::Deref for Semantics<'db, DB> {
    type Target = SemanticsImpl<'db>;

    fn deref(&self) -> &Self::Target {
        &self.impl_
    }
}

impl<DB: HirDb> Semantics<'_, DB> {
    pub fn find_node_at_offset<'a, N: AstNode<'a>>(
        &self,
        node: SyntaxNode<'a>,
        offset: TextSize,
    ) -> Option<N> {
        match node.token_or_node_at_offset(offset) {
            Either::Left(tok_at_offset) => tok_at_offset
                .map(|tok| SyntaxAncestors::start_from(tok.parent))
                .kmerge_by(|left, right| {
                    left.range()
                        .map(|left| left.end() - left.start())
                        .lt(&right.range().map(|right| right.end() - right.start()))
                })
                .find_map(N::cast),
            Either::Right(node) => SyntaxAncestors::start_from(node).find_map(N::cast),
        }
    }
}

pub struct SemanticsImpl<'db> {
    pub db: &'db dyn HirDb,

    // s2d_cache
    // Root -> HirFileId
    root2file_cache: RefCell<FxHashMap<SyntaxNode<'db>, HirFileId>>,
    source2def_cache: RefCell<Source2DefCache<'db>>,
    hir2def_cache: RefCell<Hir2DefCache>,
}

impl<'db> SemanticsImpl<'db> {
    fn new(db: &'db dyn HirDb) -> Self {
        SemanticsImpl {
            db,
            root2file_cache: Default::default(),
            source2def_cache: Default::default(),
            hir2def_cache: Default::default(),
        }
    }

    pub fn parse_root(&self, file_id: FileId) -> Option<SyntaxNode<'_>> {
        let tree = self.db.parse_src(file_id);

        // Unsafe: we garentee that the root node is valid for the lifetime of the db
        let root = tree.root()?;
        let root_node: SyntaxNode<'db> =
            unsafe { std::mem::transmute::<SyntaxNode<'_>, SyntaxNode<'db>>(root) };
        self.cache_node2file(root_node, file_id.into());
        Some(root_node)
    }

    pub fn parse(&self, file_id: FileId) -> Option<ast::CompilationUnit<'_>> {
        ast::CompilationUnit::cast(self.parse_root(file_id)?)
    }

    pub fn find_file(&self, node: SyntaxNode) -> Option<HirFileId> {
        let root_node = node.find_root();
        self.lookup_file_id(root_node)
    }

    pub fn container_for_node(&self, node: SyntaxNode) -> Option<ContainerId> {
        let file_id = self.find_file(node)?;
        self.with_ctx(|ctx| Some(ctx.find_container(InFile::new(file_id, node))))
    }

    fn cache_node2file(&self, root_node: SyntaxNode<'db>, file_id: HirFileId) {
        debug_assert!(root_node.parent().is_none());
        let mut cache = self.root2file_cache.borrow_mut();
        let prev = cache.insert(root_node, file_id);
        debug_assert!(prev.is_none() || prev == Some(file_id))
    }

    fn lookup_file_id(&self, root_node: SyntaxNode) -> Option<HirFileId> {
        let cache = self.root2file_cache.borrow();
        cache.get(&root_node).copied()
    }

    fn with_ctx<F: FnOnce(&mut Source2DefCtx<'_, '_>) -> T, T>(&self, f: F) -> T {
        let mut ctx = Source2DefCtx {
            db: self.db,
            source_cache: &mut self.source2def_cache.borrow_mut(),
            hir_cache: &mut self.hir2def_cache.borrow_mut(),
        };
        f(&mut ctx)
    }
}

impl SemanticsImpl<'_> {
    pub fn block_to_def(&self, block: ast::BlockStatement) -> Option<BlockId> {
        let file_id = self.find_file(block.syntax())?;
        let block_src = BlockSrc::from(block);
        self.with_ctx(|ctx| ctx.block_to_def(InFile::new(file_id, block_src)))
    }

    pub fn subroutine_to_def(&self, subroutine: ast::FunctionDeclaration) -> Option<SubroutineId> {
        let file_id = self.find_file(subroutine.syntax())?;
        let subroutine_src = SubroutineSrc::from(subroutine);
        self.with_ctx(|ctx| ctx.subroutine_to_def(InFile::new(file_id, subroutine_src)))
    }

    pub fn expr_to_def(&self, in_cont: InContainer<ExprId>) -> Option<PathResolution> {
        self.with_ctx(|ctx| ctx.expr_to_def(in_cont))
    }

    pub fn name_to_def(&self, in_cont: InContainer<Ident>) -> Option<PathResolution> {
        self.with_ctx(|ctx| ctx.name_to_def(in_cont))
    }
}
