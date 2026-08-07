use std::ops;

use hir_def::{
    Ident,
    container::{ArenaOwnerId, InContainer, InFile, SubroutineScope},
    db::HirDefDb,
    def_id::DefId,
    expr::ExprId,
    module::ModuleId,
    owner::OwnerId,
    symbol::{NameContext, Resolution},
};
use itertools::{Either, Itertools};
use preproc_expand::file::HirFileId;
use syntax::{
    SyntaxAncestors, SyntaxNode, SyntaxNodeExt, SyntaxTree,
    ast::{self, AstNode},
};
use utils::text_edit::TextSize;
use vfs::FileId;

mod hir_to_def;
pub mod pathres;
pub mod resolver;
mod source_to_def;

pub use source_to_def::is_generate_branch_member;

pub struct Semantics<'db, DB> {
    pub db: &'db DB,
    impl_: SemanticsImpl<'db>,
}

pub struct ParsedFile {
    file_id: HirFileId,
    tree: SyntaxTree,
}

impl ParsedFile {
    pub fn file_id(&self) -> HirFileId {
        self.file_id
    }

    pub fn syntax_tree(&self) -> &SyntaxTree {
        &self.tree
    }

    pub fn root(&self) -> Option<SyntaxNode<'_>> {
        self.tree.root()
    }

    pub fn compilation_unit(&self) -> Option<ast::CompilationUnit<'_>> {
        ast::CompilationUnit::cast(self.root()?)
    }
}

impl<DB: HirDefDb> Semantics<'_, DB> {
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

impl<DB: HirDefDb> Semantics<'_, DB> {
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
    pub db: &'db dyn HirDefDb,
}

impl<'db> SemanticsImpl<'db> {
    pub fn new(db: &'db dyn HirDefDb) -> Self {
        SemanticsImpl { db }
    }

    pub fn parse_file(&self, file_id: FileId) -> ParsedFile {
        let file_id = file_id.into();
        ParsedFile { file_id, tree: self.db.parse(file_id) }
    }

    pub fn container_for_node(&self, file_id: HirFileId, node: SyntaxNode) -> Option<ArenaOwnerId> {
        Some(source_to_def::find_container(self.db, InFile::new(file_id, node)))
    }
}

impl SemanticsImpl<'_> {
    pub fn module_to_def(
        &self,
        file_id: HirFileId,
        module: ast::ModuleDeclaration,
    ) -> Option<ModuleId> {
        source_to_def::module_to_def(self.db, file_id, module)
    }

    pub fn block_to_def(&self, file_id: HirFileId, block: ast::BlockStatement) -> Option<OwnerId> {
        source_to_def::block_to_def(self.db, file_id, block)
    }

    pub fn subroutine_to_def(
        &self,
        file_id: HirFileId,
        subroutine: ast::FunctionDeclaration,
    ) -> Option<SubroutineScope> {
        source_to_def::subroutine_to_def(self.db, file_id, subroutine)
    }

    pub fn expr_to_def(&self, in_cont: InContainer<ExprId>) -> Resolution<DefId> {
        hir_to_def::expr_to_def(self.db, in_cont)
    }

    pub fn name_to_def(&self, in_cont: InContainer<Ident>) -> Resolution<DefId> {
        hir_to_def::name_to_def(self.db, in_cont, NameContext::Value)
    }
}
