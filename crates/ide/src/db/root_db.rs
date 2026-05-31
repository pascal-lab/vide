use std::{fmt, mem::ManuallyDrop};

use hir::{
    base_db::{
        diagnostics_config::DiagnosticsConfig,
        project::ProjectConfig,
        salsa::{self, Durability},
        source_db::{
            FileLoader, ParseSrcForCompilationQuery, ParseSrcQuery, SourceDb, SourceDbStorage,
            SourceRootDb, SourceRootDbStorage,
        },
    },
    db::{
        BlockQuery, BlockScopeQuery, BlockWithSourceMapQuery, FileScopeQuery, HirDbStorage,
        HirFileQuery, HirFileWithSourceMapQuery, InternDbStorage, ModuleQuery, ModuleScopeQuery,
        ModuleWithSourceMapQuery,
    },
};
use triomphe::Arc;
use vfs::{FileId, anchored_path::AnchoredPath};

use crate::db::{
    line_index_db::LineIndexDbStorage, workspace_symbol_index_db::WorkspaceSymbolIndexDbStorage,
};

#[salsa::database(
    SourceDbStorage,
    SourceRootDbStorage,
    HirDbStorage,
    InternDbStorage,
    LineIndexDbStorage,
    WorkspaceSymbolIndexDbStorage
)]
pub struct RootDb {
    // `ManuallyDrop` is used to avoid duplicating drop glue like `Weak::drop'
    // for improved compile times and performance.
    storage: ManuallyDrop<salsa::Storage<Self>>,
}

impl salsa::Database for RootDb {}

impl Drop for RootDb {
    fn drop(&mut self) {
        unsafe { ManuallyDrop::drop(&mut self.storage) };
    }
}

impl fmt::Debug for RootDb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RootDb").finish()
    }
}

impl FileLoader for RootDb {
    fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let source_root_id = SourceRootDb::source_root_id(self, path.anchor_id);
        let source_root = SourceRootDb::source_root(self, source_root_id);
        source_root.resolve_path(path)
    }
}

pub const DEFAULT_PARSE_LRU_CAP: usize = 128;

impl RootDb {
    pub fn new(lru_capacity: Option<usize>) -> RootDb {
        let mut db = RootDb { storage: ManuallyDrop::new(salsa::Storage::default()) };
        db.set_files_with_durability(Default::default(), Durability::HIGH);
        db.set_diagnostics_config_with_durability(
            Arc::new(DiagnosticsConfig::default()),
            Durability::HIGH,
        );
        db.set_project_config_with_durability(Arc::new(ProjectConfig::default()), Durability::HIGH);
        db.update_parse_query_lru_capacity(lru_capacity);
        db
    }

    pub fn update_parse_query_lru_capacity(&mut self, lru_capacity: Option<usize>) {
        let lru_capacity = lru_capacity.unwrap_or(DEFAULT_PARSE_LRU_CAP);
        ParseSrcQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        ParseSrcForCompilationQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        HirFileWithSourceMapQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        ModuleWithSourceMapQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        BlockWithSourceMapQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        HirFileQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        ModuleQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        BlockQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        FileScopeQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        ModuleScopeQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
        BlockScopeQuery.in_db_mut(self).set_lru_capacity(lru_capacity);
    }
}

impl salsa::ParallelDatabase for RootDb {
    fn snapshot(&self) -> salsa::Snapshot<RootDb> {
        salsa::Snapshot::new(RootDb { storage: ManuallyDrop::new(self.storage.snapshot()) })
    }
}
