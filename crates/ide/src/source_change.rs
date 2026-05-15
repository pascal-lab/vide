use std::{collections::hash_map::Entry, mem};

use nohash_hasher::IntMap;
use utils::text_edit::{TextEdit, TextEditBuilder, TextRange, TextSize};
use vfs::FileId;

#[derive(Default, Debug, Clone)]
pub struct SourceChange {
    pub text_edits: IntMap<FileId, TextEdit>,
}

impl SourceChange {
    pub fn insert_text_edit(&mut self, file_id: FileId, edit: TextEdit) -> Result<(), TextEdit> {
        match self.text_edits.entry(file_id) {
            Entry::Occupied(mut e) => e.get_mut().union(edit),
            Entry::Vacant(e) => {
                e.insert(edit);
                Ok(())
            }
        }
    }
}

pub struct SourceChangeBuilder {
    pub edit: TextEditBuilder,
    pub file_id: FileId,
    pub source_change: SourceChange,
}

impl SourceChangeBuilder {
    pub fn new(file_id: impl Into<FileId>) -> SourceChangeBuilder {
        SourceChangeBuilder {
            edit: TextEdit::builder(),
            file_id: file_id.into(),
            source_change: SourceChange::default(),
        }
    }

    pub fn edit_file(&mut self, file_id: impl Into<FileId>) {
        self.commit();
        self.file_id = file_id.into();
    }

    pub fn delete(&mut self, range: TextRange) {
        self.edit.delete(range)
    }

    pub fn insert(&mut self, offset: TextSize, text: impl Into<String>) {
        self.edit.insert(offset, text.into())
    }

    pub fn replace(&mut self, range: TextRange, replace_with: impl Into<String>) {
        self.edit.replace(range, replace_with.into())
    }

    fn commit(&mut self) {
        let edit = mem::replace(&mut self.edit, TextEdit::builder());
        let _ = self.source_change.insert_text_edit(self.file_id, edit.finish());
    }

    pub fn finish(mut self) -> SourceChange {
        self.commit();
        mem::take(&mut self.source_change)
    }
}
