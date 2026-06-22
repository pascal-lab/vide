//! Source-level semantic facade over `hir`.
//!
//! This crate is the stable entry point that IDE features should use when they
//! need to answer questions that start from source syntax or editor positions.
//! It intentionally delegates to the existing `hir::semantics` implementation
//! today, while giving the repository a real boundary that can absorb future
//! source-to-def, def-to-source, and occurrence-validation APIs without making
//! IDE features depend on HIR internals directly.

pub use hir::semantics::{ParsedFile, pathres::PathResolution};
use hir::{
    container::{ContainerId, InContainer},
    db::HirDb,
    file::HirFileId,
    hir_def::{Ident, block::BlockId, expr::ExprId, module::ModuleId, subroutine::SubroutineId},
};
use syntax::{SyntaxNode, ast};
use utils::text_edit::TextSize;
use vfs::FileId;

/// Source-level semantic facade used by IDE features.
///
/// The wrapper is deliberately small but functional: it exposes the same
/// source-facing operations that IDE code currently uses from
/// `hir::semantics::Semantics`. New IDE code should depend on this crate
/// instead of importing `hir::semantics` directly.
pub struct Semantics<'db, DB: HirDb> {
    inner: hir::semantics::Semantics<'db, DB>,
}

impl<'db, DB: HirDb> Semantics<'db, DB> {
    pub fn new(db: &'db DB) -> Self {
        Self { inner: hir::semantics::Semantics::new(db) }
    }

    pub fn db(&self) -> &'db DB {
        self.inner.db
    }

    pub fn parse_file(&self, file_id: FileId) -> ParsedFile {
        self.inner.parse_file(file_id)
    }

    pub fn find_node_at_offset<'a, N: ast::AstNode<'a>>(
        &self,
        node: SyntaxNode<'a>,
        offset: TextSize,
    ) -> Option<N> {
        self.inner.find_node_at_offset(node, offset)
    }

    pub fn container_for_node(
        &self,
        file_id: HirFileId,
        node: SyntaxNode<'_>,
    ) -> Option<ContainerId> {
        self.inner.container_for_node(file_id, node)
    }

    pub fn module_to_def(
        &self,
        file_id: HirFileId,
        module: ast::ModuleDeclaration<'_>,
    ) -> Option<ModuleId> {
        self.inner.module_to_def(file_id, module)
    }

    pub fn block_to_def(
        &self,
        file_id: HirFileId,
        block: ast::BlockStatement<'_>,
    ) -> Option<BlockId> {
        self.inner.block_to_def(file_id, block)
    }

    pub fn subroutine_to_def(
        &self,
        file_id: HirFileId,
        subroutine: ast::FunctionDeclaration<'_>,
    ) -> Option<SubroutineId> {
        self.inner.subroutine_to_def(file_id, subroutine)
    }

    pub fn expr_to_def(&self, in_cont: InContainer<ExprId>) -> Option<PathResolution> {
        self.inner.expr_to_def(in_cont)
    }

    pub fn name_to_def(&self, in_cont: InContainer<Ident>) -> Option<PathResolution> {
        self.inner.name_to_def(in_cont)
    }

    /// Escape hatch for code that has not yet been migrated. Avoid using this
    /// in new code; prefer adding explicit facade methods instead.
    pub fn as_hir_semantics(&self) -> &hir::semantics::Semantics<'db, DB> {
        &self.inner
    }
}

impl<'db, DB: HirDb> std::ops::Deref for Semantics<'db, DB> {
    type Target = hir::semantics::Semantics<'db, DB>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use hir::{
        base_db::{
            diagnostics_config::DiagnosticsConfig,
            project::{PreprocessConfig, ProjectConfig},
            salsa::{self, Durability},
            source_db::{
                FileLoader, SourceDb, SourceDbStorage, SourceFileKind, SourceRootDb,
                SourceRootDbStorage,
            },
            source_root::{SourceRoot, SourceRootId},
        },
        db::{HirDbStorage, InternDbStorage},
    };
    use rustc_hash::FxHashSet;
    use syntax::ast::ModuleDeclaration;
    use triomphe::Arc;
    use utils::{
        line_index::TextSize,
        paths::{AbsPathBuf, Utf8PathBuf},
    };
    use vfs::{FileId, FileSet, VfsPath, anchored_path::AnchoredPath};

    use super::Semantics;

    const FILE: FileId = FileId(0);
    const ROOT: SourceRootId = SourceRootId(0);

    #[salsa::database(SourceDbStorage, SourceRootDbStorage, InternDbStorage, HirDbStorage)]
    #[derive(Default)]
    struct TestDb {
        storage: salsa::Storage<Self>,
    }

    impl salsa::Database for TestDb {}

    impl fmt::Debug for TestDb {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDb").finish()
        }
    }

    impl FileLoader for TestDb {
        fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
            let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
            SourceRootDb::source_root(self, source_root_id).resolve_path(path)
        }
    }

    fn db_with_file(text: &str) -> TestDb {
        let path = abs_path("top.sv");
        let vfs_path = VfsPath::from(path.clone());

        let mut file_set = FileSet::default();
        file_set.insert(FILE, vfs_path.clone());
        let root = SourceRoot::new_local(file_set);

        let mut files = FxHashSet::default();
        files.insert(FILE);

        let mut db = TestDb::default();
        db.set_files_with_durability(Box::new(files), Durability::HIGH);
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_source_root_with_durability(ROOT, Arc::new(root), Durability::LOW);
        db.set_source_root_id_with_durability(FILE, ROOT, Durability::LOW);
        db.set_file_path_with_durability(FILE, Some(path), Durability::LOW);
        db.set_file_kind_with_durability(
            FILE,
            SourceFileKind::from_path(&vfs_path),
            Durability::LOW,
        );
        db.set_file_text_with_durability(FILE, Arc::from(text), Durability::LOW);
        db.set_file_preprocess_config_with_durability(
            FILE,
            Arc::new(PreprocessConfig::default()),
            Durability::LOW,
        );
        db
    }

    fn abs_path(path: &str) -> AbsPathBuf {
        let prefix = if cfg!(windows) { "C:/repo" } else { "/repo" };
        AbsPathBuf::assert(Utf8PathBuf::from(format!("{prefix}/{path}")))
    }

    fn offset(text: &str, needle: &str) -> TextSize {
        TextSize::from(u32::try_from(text.find(needle).unwrap()).unwrap())
    }

    #[test]
    fn facade_parses_file_and_finds_source_node() {
        let text = "module top;\n  wire sig;\nendmodule\n";
        let db = db_with_file(text);
        let sema = Semantics::new(&db);

        let parsed = sema.parse_file(FILE);
        let root = parsed.root().expect("fixture should parse");
        let module = sema
            .find_node_at_offset::<ModuleDeclaration>(root, offset(text, "top"))
            .expect("module declaration should be found at module name");

        let module_id = sema.module_to_def(FILE.into(), module);
        assert!(module_id.is_some(), "facade should resolve source module to HIR definition");
    }
}
