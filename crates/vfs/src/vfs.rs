use std::{fmt, hash::BuildHasherDefault, mem};

use crate::{
    loader::{VfsLoadError, VfsLoadResult},
    vfs_path::VfsPath,
};
use indexmap::IndexSet;
use rustc_hash::FxHasher;
use utils::{lines::LineEnding, text_edit::SourceEditKind};

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileId(pub u32);

impl nohash_hasher::IsEnabled for FileId {}

#[derive(Default)]
pub struct Vfs {
    paths: IndexSet<VfsPath, BuildHasherDefault<FxHasher>>,
    file_states: Vec<FileState>,
    changes: Vec<ChangedFile>,
}

#[derive(Copy, PartialEq, PartialOrd, Clone)]
pub enum FileState {
    Exists,
    Deleted,
}

#[derive(Debug)]
pub struct ChangedFile {
    pub file_id: FileId,
    pub change_kind: ChangeKind,
}

impl ChangedFile {
    pub fn file_state(&self) -> FileState {
        match &self.change_kind {
            ChangeKind::Create(_) | ChangeKind::Modify(_, _) => FileState::Exists,
            ChangeKind::Delete => FileState::Deleted,
        }
    }

    pub fn is_created_or_deleted(&self) -> bool {
        matches!(self.change_kind, ChangeKind::Create(_) | ChangeKind::Delete)
    }

    pub fn exists(&self) -> bool {
        self.file_state() == FileState::Exists
    }

    pub fn source_edits(&self) -> Option<&SourceEditKind> {
        match &self.change_kind {
            ChangeKind::Create(_) | ChangeKind::Delete => None,
            ChangeKind::Modify(_, edits) => Some(edits),
        }
    }

    pub fn get_line_endings(&self) -> Option<LineEnding> {
        match self.change_kind {
            ChangeKind::Create(Ok((_, e))) | ChangeKind::Modify(Ok((_, e)), _) => Some(e),
            _ => None,
        }
    }

    pub fn get_text(self) -> Option<String> {
        match self.change_kind {
            ChangeKind::Create(Ok((text, _))) | ChangeKind::Modify(Ok((text, _)), _) => Some(text),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChangeKind {
    Create(VfsLoadResult),
    Modify(VfsLoadResult, SourceEditKind),
    Delete,
}

impl Vfs {
    pub fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.paths.get_index_of(path).map(|id| FileId(id as u32)).filter(|id| self.exists(*id))
    }

    pub fn file_path(&self, file_id: FileId) -> &VfsPath {
        self.paths.get_index(file_id.0 as usize).unwrap()
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &VfsPath)> + '_ {
        (0..self.file_states.len())
            .map(|it| FileId(it as u32))
            .filter(move |&file_id| self.exists(file_id))
            .map(move |file_id| (file_id, self.file_path(file_id)))
    }

    pub fn set_file_contents(
        &mut self,
        path: &VfsPath,
        contents: VfsLoadResult,
        source_edits: SourceEditKind,
    ) {
        let file_id = self.file_id_or_alloc(path);
        let change_kind = match (self.file_state(file_id), contents) {
            (FileState::Exists, Err(VfsLoadError::LoadError)) => ChangeKind::Delete,
            (FileState::Exists, v) => ChangeKind::Modify(v, source_edits),
            (FileState::Deleted, Err(VfsLoadError::LoadError)) => return,
            (FileState::Deleted, v) => ChangeKind::Create(v),
        };

        if let ChangeKind::Modify(_, SourceEditKind::Edits(edits)) = &change_kind
            && edits.is_empty()
        {
            return;
        }

        let change_file = ChangedFile { file_id, change_kind };
        self.file_states[file_id.0 as usize] = change_file.file_state();
        self.changes.push(change_file);
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn take_changes(&mut self) -> Vec<ChangedFile> {
        mem::take(&mut self.changes)
    }

    pub fn exists(&self, file_id: FileId) -> bool {
        self.file_state(file_id) == FileState::Exists
    }

    fn file_id_or_alloc(&mut self, path: &VfsPath) -> FileId {
        let id = match self.paths.get_index_of(path) {
            Some(id) => id,
            None => {
                let path = path.clone();
                self.paths.insert_full(path).0
            }
        };
        assert!(id < u32::MAX as usize);

        let len = self.file_states.len().max(id + 1);
        self.file_states.resize(len, FileState::Deleted);
        FileId(id as u32)
    }

    fn file_state(&self, file_id: FileId) -> FileState {
        self.file_states[file_id.0 as usize]
    }
}

impl fmt::Debug for Vfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vfs").field("n_files", &self.file_states.len()).finish()
    }
}
