use std::ops::Range;

use base_db::{salsa, source_db::SourceDb};
use line_index::{LineIndex, TextRange};
use triomphe::Arc;
use vfs::FileId;

#[salsa::query_group(LineIndexDbStorage)]
pub trait LineIndexDb: SourceDb {
    fn line_index(&self, file_id: FileId) -> Arc<LineIndex>;
}

fn line_index(db: &dyn LineIndexDb, file_id: FileId) -> Arc<LineIndex> {
    let text = db.file_text(file_id);
    Arc::new(LineIndex::new(&text))
}

pub trait LineIndexExt {
    fn line_ranges(&self, range: TextRange) -> Range<usize>;
}

impl LineIndexExt for LineIndex {
    #[inline]
    fn line_ranges(&self, range: TextRange) -> Range<usize> {
        // TODO: calculate only lines
        let start = self.line_col(range.start());
        let end = self.line_col(range.end().checked_sub(1.into()).unwrap());
        (start.line as usize)..(end.line as usize + 1)
    }
}
