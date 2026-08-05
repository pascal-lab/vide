use base_db::{salsa, source_db::SourceDb};
use triomphe::Arc;
use utils::line_index::LineIndex;
use vfs::FileId;

#[salsa::db]
pub trait LineIndexDb: SourceDb {}

impl dyn LineIndexDb + '_ {
    pub fn line_index(&self, file_id: FileId) -> Arc<LineIndex> {
        line_index(self, file_id, ())
    }
}

#[salsa::tracked(returns(clone), unsafe(non_salsa_values))]
fn line_index(db: &dyn LineIndexDb, file_id: FileId, _key: ()) -> Arc<LineIndex> {
    let text = db.file_text(file_id);
    Arc::new(LineIndex::new(&text))
}
