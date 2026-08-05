use base_db::{salsa, source_db::SourceRootDb, source_root::SourceRootId};
use hir_ty::db::TyDb;
use triomphe::Arc;
use vfs::FileId;

use crate::{
    semantic_index::{ModuleIndex, SemanticIndex},
    workspace_symbols::{SymbolIndex, WorkspaceSymbol},
};

#[salsa::query_group(WorkspaceSymbolIndexDbStorage)]
pub trait WorkspaceSymbolIndexDb: SourceRootDb + TyDb {
    fn file_workspace_symbols(&self, file_id: FileId) -> Arc<[WorkspaceSymbol]>;
    fn source_root_symbol_index(&self, source_root_id: SourceRootId) -> Arc<SymbolIndex>;
    fn source_root_module_index(&self, source_root_id: SourceRootId) -> Arc<ModuleIndex>;
    fn source_root_semantic_index(&self, source_root_id: SourceRootId) -> Arc<SemanticIndex>;
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

fn source_root_module_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<ModuleIndex> {
    Arc::new(ModuleIndex::for_source_root(db, source_root_id))
}

fn source_root_semantic_index(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<SemanticIndex> {
    Arc::new(SemanticIndex::for_source_root(db, source_root_id))
}

pub(crate) fn source_root_symbol_index_for_root(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<SymbolIndex> {
    WorkspaceSymbolIndexDb::source_root_symbol_index(db, source_root_id)
}

pub(crate) fn source_root_module_index_for_root(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<ModuleIndex> {
    WorkspaceSymbolIndexDb::source_root_module_index(db, source_root_id)
}

pub(crate) fn source_root_semantic_index_for_root(
    db: &dyn WorkspaceSymbolIndexDb,
    source_root_id: SourceRootId,
) -> Arc<SemanticIndex> {
    WorkspaceSymbolIndexDb::source_root_semantic_index(db, source_root_id)
}
