use crate::{
    file::HirFileId,
    hir_def::{
        self,
        module::{self, ModuleDecl, ModuleSourceMap},
        FileSourceMap, HirFile, ModuleId,
    },
    scope::{ModuleScope, UnitScope},
};
use base_db::source_db::SourceDb;
use syntax::parse::SyntaxTree;
use triomphe::Arc;

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

    fn module(&self, module_id: ModuleId) -> Arc<ModuleDecl>;

    #[salsa::invoke(UnitScope::unit_scope_query)]
    fn unit_scope(&self) -> Arc<UnitScope>;

    #[salsa::invoke(ModuleScope::module_scope_query)]
    fn module_scope(&self, module_id: ModuleId) -> Arc<ModuleScope>;
}

pub fn hir_syntax_tree(db: &dyn HirDb, file_id: HirFileId) -> Option<SyntaxTree> {
    db.syntax_tree(file_id.0)
}

pub fn hir_file_text(db: &dyn HirDb, file_id: HirFileId) -> Arc<str> {
    db.file_text(file_id.0)
}

fn hir_file(db: &dyn HirDb, file_id: HirFileId) -> Arc<HirFile> {
    db.hir_file_with_source_map(file_id).0
}

fn module(db: &dyn HirDb, module_id: ModuleId) -> Arc<ModuleDecl> {
    db.module_with_source_map(module_id).0
}
