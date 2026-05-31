use hir::{
    base_db::{salsa, source_db::SourceRootDb, source_root::SourceRootId},
    db::HirDb,
};
use triomphe::Arc;
use vfs::FileId;

use crate::{
    db::root_db::RootDb,
    workspace_symbols::{SymbolIndex, WorkspaceSymbol},
};

#[salsa::query_group(WorkspaceSymbolIndexDbStorage)]
pub trait WorkspaceSymbolIndexDb: SourceRootDb + HirDb {
    fn file_workspace_symbols(&self, file_id: FileId) -> Arc<[WorkspaceSymbol]>;
    fn source_root_symbol_index(&self, source_root_id: SourceRootId) -> Arc<SymbolIndex>;
}

fn file_workspace_symbols(
    db: &dyn WorkspaceSymbolIndexDb,
    file_id: FileId,
) -> Arc<[WorkspaceSymbol]> {
    crate::workspace_symbols::file_symbols(db, file_id)
}

fn source_root_symbol_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<SymbolIndex> {
    Arc::new(SymbolIndex::for_source_root(db, source_root_id))
}

pub(crate) fn source_root_symbol_index_for_root(
    db: &RootDb,
    source_root_id: SourceRootId,
) -> Arc<SymbolIndex> {
    WorkspaceSymbolIndexDb::source_root_symbol_index(db, source_root_id)
}
