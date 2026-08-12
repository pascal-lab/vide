use base_db::{salsa, source_db::SourceDb};
use super::SourceFileQueryKey;
use triomphe::Arc;
use utils::line_index::LineIndex;
use vfs::FileId;

#[salsa::db]
pub trait LineIndexDb: SourceDb {
    fn line_index(&self, file_id: FileId) -> Arc<LineIndex>
    where
        Self: Sized,
    {
        line_index(self, SourceFileQueryKey::new(self, file_id))
    }
}

#[salsa::tracked(lru = 256, returns(clone))]
fn line_index(db: &dyn LineIndexDb, key: SourceFileQueryKey) -> Arc<LineIndex> {
    let file_id = key.file_id(db);
    let text = db.file_text(file_id);
    Arc::new(LineIndex::new(&text))
}