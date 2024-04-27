use std::{cell::RefCell, ops};

use itertools::Itertools;
use rustc_hash::FxHashMap;
use syntax::{
    ast::{self, AstNode}, treesit_ext::find_root, SyntaxNode
};
use vfs::vfs::FileId;

use crate::{db::HirDb, file::HirFileId};

use self::source_to_def::Source2DefCtx;

mod source_to_def;

pub struct Semantics<'db, DB> {
    db: &'db DB,
    impl_: SemanticsImpl<'db>,
}

impl<'db, DB: HirDb> Semantics<'db, DB> {
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

pub struct SemanticsImpl<'db> {
    pub db: &'db dyn HirDb,
    // s2d_cache
    // Root -> HirFileId
    root2file_cache: RefCell<FxHashMap<SyntaxNode<'db>, HirFileId>>,
}

impl<'db> SemanticsImpl<'db> {
    fn new(db: &'db dyn HirDb) -> Self {
        SemanticsImpl { db, root2file_cache: Default::default() }
    }

    pub fn parse(&self, file_id: FileId) -> ast::SourceFile {
        let tree = self.db.syntax_tree(file_id).expect("missing syntax tree");
        // Unsafe: we garentee that the root node is valid for the lifetime of the db
        let root_node =
            unsafe { std::mem::transmute::<SyntaxNode<'_>, SyntaxNode<'db>>(tree.root_node()) };
        self.cache_node2file(root_node.clone(), file_id.into());
        ast::SourceFile::cast(root_node).unwrap()
    }

    fn cache_node2file(&self, root_node: SyntaxNode<'db>, file_id: HirFileId) {
        assert!(root_node.parent().is_none());
        let mut cache = self.root2file_cache.borrow_mut();
        let prev = cache.insert(root_node, file_id);
        assert!(prev.is_none() || prev == Some(file_id))
    }

    fn lookup(&self, root_node: &SyntaxNode) -> Option<HirFileId> {
        let cache = self.root2file_cache.borrow();
        cache.get(root_node).copied()
    }

    fn find_file<'a>(&self, node: &SyntaxNode<'a>) -> HirFileId {
        let root_node = find_root(node.clone());
        let file_id = self.lookup(&root_node).unwrap_or_else(|| {
            panic!(
                "\n\nFailed to lookup {:?}.\nroot node:   {:?}\nknown nodes: {}\n\n",
                node,
                root_node,
                self.root2file_cache
                    .borrow()
                    .keys()
                    .map(|it| format!("{it:?}"))
                    .join(", ")
            )
        });
        file_id
    }

    fn with_ctx<F: FnOnce(&mut Source2DefCtx<'_>) -> T, T>(&self, f: F) -> T {
        let mut ctx = Source2DefCtx { db: self.db };
        f(&mut ctx)
    }
}
