use hir::{
    base_db::{salsa, source_db::SourceRootDb},
    db::HirDb,
};
use index::FileIndex;
use triomphe::Arc;
use vfs::FileId;

#[salsa::query_group(WorkspaceSymbolIndexDbStorage)]
pub trait WorkspaceSymbolIndexDb: SourceRootDb + HirDb {
    fn file_index(&self, file_id: FileId) -> Arc<FileIndex>;
}

fn file_index(db: &dyn WorkspaceSymbolIndexDb, file_id: FileId) -> Arc<FileIndex> {
    Arc::new(crate::workspace_symbols::file_index(db, file_id))
}
