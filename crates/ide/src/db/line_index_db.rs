use base_db::{salsa, source_db::SourceDb};
use triomphe::Arc;
use utils::line_index::LineIndex;
use vfs::FileId;

use super::SourceFileQueryKey;

#[salsa::db]
pub trait LineIndexDb: SourceDb {
    fn line_index(&self, file_id: FileId) -> Arc<LineIndex>
    where
        Self: Sized,
    {
        line_index(self, SourceFileQueryKey::new(self, file_id))
    }
}

#[salsa::tracked(returns(clone))]
fn line_index(db: &dyn LineIndexDb, key: SourceFileQueryKey) -> Arc<LineIndex> {
    let text = db.file_text(key.file_id(db));
    Arc::new(LineIndex::new(&text))
}
