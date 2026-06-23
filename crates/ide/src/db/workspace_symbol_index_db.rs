use hir::{
    base_db::{salsa, source_db::SourceRootDb, source_root::SourceRootId},
    db::HirDb,
};
use index::{FileIndex, ProjectIndex};
use triomphe::Arc;
use vfs::FileId;

#[salsa::query_group(WorkspaceSymbolIndexDbStorage)]
pub trait WorkspaceSymbolIndexDb: SourceRootDb + HirDb {
    fn file_index(&self, file_id: FileId) -> Arc<FileIndex>;
    fn source_root_project_index(&self, source_root_id: SourceRootId) -> Arc<ProjectIndex>;
}

fn file_index(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Arc<FileIndex> {
    Arc::new(crate::workspace_symbols::file_index(db, file_id))
}

fn source_root_project_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<ProjectIndex> {
    Arc::new(crate::workspace_symbols::source_root_project_index(db, source_root_id))
}
