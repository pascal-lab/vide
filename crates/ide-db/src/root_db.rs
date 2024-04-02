use std::{fmt, mem::ManuallyDrop};

use base_db::{
    self,
    salsa::{self, Durability},
    source_db::{FileLoader, SourceDb, SourceRootDb},
};
use rustc_hash::FxHashSet;
use triomphe::Arc;
use vfs::{anchored_path::AnchoredPath, vfs::FileId};

use crate::line_index_db::LineIndexDbStorage;

#[salsa::database(
    base_db::source_db::SourceDbStorage,
    base_db::source_db::SourceRootDbStorage,
    LineIndexDbStorage
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
    fn file_text(&self, file_id: FileId) -> Arc<str> {
        SourceRootDb::file_text(self, file_id)
    }

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
        db.update_parse_query_lru_capacity(lru_capacity);
        db
    }

    // TODO
    pub fn update_parse_query_lru_capacity(&mut self, lru_capacity: Option<usize>) {
        let lru_capacity = lru_capacity.unwrap_or(DEFAULT_PARSE_LRU_CAP);
    }
}

impl salsa::ParallelDatabase for RootDb {
    fn snapshot(&self) -> salsa::Snapshot<RootDb> {
        salsa::Snapshot::new(RootDb { storage: ManuallyDrop::new(self.storage.snapshot()) })
    }
}
