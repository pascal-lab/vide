use triomphe::Arc;
use std::fmt;

use vfs::FileId;

use crate::source_root::SourceRoot;

#[derive(Debug, Default)]
pub struct Change {
    pub roots: Option<Vec<SourceRoot>>,
    pub changed_files: Vec<(FileId, Option<Arc<str>>)>,
}

impl Change {
    pub fn new() -> Self {
        Change::default()
    }

    pub fn set_roots(&mut self, roots: Vec<SourceRoot>) {
        self.roots = Some(roots);
    }

    pub fn add_changed_file(&mut self, file_id: FileId, new_text: Option<Arc<str>>) {
        self.changed_files.push((file_id, new_text))
    }
}
