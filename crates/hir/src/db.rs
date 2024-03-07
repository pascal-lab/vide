use crate::{
    hir_def::{
        self,
        module::{self, ModuleDecl, ModuleSourceMap},
        FileSourceMap, HirFile, ModuleId,
    },
    HirFileId,
};
use base_db::source_db::SourceDb;
use syntax::parse::SyntaxTree;
use triomphe::Arc;
use vfs::vfs::FileId;

#[salsa::query_group(HirDbStorage)]
pub trait HirDb: SourceDb {
    fn hir_syntax_tree(&self, file_id: HirFileId) -> Option<SyntaxTree>;

    fn hir_file_text(&self, file_id: HirFileId) -> Arc<str>;

    #[salsa::invoke(hir_def::hir_file_with_source_map_query)]
    fn hir_file_with_source_map(&self, file_id: HirFileId) -> (Arc<HirFile>, Arc<FileSourceMap>);

    fn hir_file(&self, file_id: HirFileId) -> Arc<HirFile>;

    #[salsa::invoke(module::module_with_source_map_query)]
    fn module_with_source_map(
        &self,
        module_id: ModuleId,
    ) -> (Arc<ModuleDecl>, Arc<ModuleSourceMap>);
}

pub fn hir_syntax_tree(db: &dyn HirDb, file_id: HirFileId) -> Option<SyntaxTree> {
    db.syntax_tree(file_id.0)
}

pub fn hir_file_text(db: &dyn HirDb, file_id: HirFileId) -> Arc<str> {
    db.file_text(file_id.0)
}

pub fn hir_file(db: &dyn HirDb, file_id: HirFileId) -> Arc<HirFile> {
    db.hir_file_with_source_map(file_id).0
}
