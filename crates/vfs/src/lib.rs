pub mod loader;
mod anchored_path;
mod vfs_path;

use std::{fmt, mem, hash::BuildHasherDefault};

pub use crate::{
    anchored_path::{AnchoredPath, AnchoredPathBuf},
    vfs_path::VfsPath,
};
use indexmap::IndexSet;
pub use utils::paths::{AbsPath, AbsPathBuf};
use rustc_hash::FxHasher;

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileId(pub u32);

impl nohash_hasher::IsEnabled for FileId {}

#[derive(Default)]
pub struct Vfs {
    paths: IndexSet<VfsPath, BuildHasherDefault<FxHasher>>,
    data: Vec<Option<Vec<u8>>>,
    changes: Vec<ChangedFile>,
}

#[derive(Debug)]
pub struct ChangedFile {
    pub file_id: FileId,
    pub change_kind: ChangeKind,
}

impl ChangedFile {
    pub fn exists(&self) -> bool {
        self.change_kind != ChangeKind::Delete
    }

    pub fn is_created_or_deleted(&self) -> bool {
        matches!(self.change_kind, ChangeKind::Create | ChangeKind::Delete)
    }
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum ChangeKind {
    Create, Modify, Delete,
}

impl Vfs {
    pub fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.paths.get_index_of(path).map(|id| FileId(id as u32))
    }

    pub fn file_path(&self, file_id: FileId) -> &VfsPath {
        self.paths.get_index(file_id.0 as usize).unwrap()
    }

    pub fn file_contents(&self, file_id: FileId) -> Option<&[u8]> {
        self.get_file_contents(file_id).as_deref()
    }

    pub fn memory_usage(&self) -> usize {
        self.data.iter().flatten().map(|d| d.capacity()).sum()
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &VfsPath)> + '_ {
        (0..self.data.len())
            .map(|it| FileId(it as u32))
            .filter(move |&file_id| self.exists(file_id))
            .map(move |file_id| (file_id, self.file_path(file_id)))
    }

    pub fn set_file_contents(&mut self, path: VfsPath, mut contents: Option<Vec<u8>>) -> bool {
        let file_id = self.file_id_or_alloc(path);
        let change_kind = match (self.get_file_contents(file_id), &contents) {
            (None, None) => return false,
            (Some(old), Some(new)) if old == new => return false,
            (None, Some(_)) => ChangeKind::Create,
            (Some(_), None) => ChangeKind::Delete,
            (Some(_), Some(_)) => ChangeKind::Modify,
        };

        if let Some(contents) = &mut contents {
            contents.shrink_to_fit();
        }

        *self.get_file_contents_mut(file_id) = contents;
        self.changes.push(ChangedFile { file_id, change_kind });
        true
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn take_changes(&mut self) -> Vec<ChangedFile> {
        mem::take(&mut self.changes)
    }

        pub fn exists(&self, file_id: FileId) -> bool {
        self.get_file_contents(file_id).is_some()
    }

    fn file_id_or_alloc(&mut self, path: VfsPath) -> FileId {
        let (id, _) = self.paths.insert_full(path);
        assert!(id < u32::MAX as usize);

        let len = self.data.len().max(id + 1);
        self.data.resize(len, None);
        FileId(id as u32)
    }

    fn get_file_contents(&self, file_id: FileId) -> &Option<Vec<u8>> {
        &self.data[file_id.0 as usize]
    }

    fn get_file_contents_mut(&mut self, file_id: FileId) -> &mut Option<Vec<u8>> {
        &mut self.data[file_id.0 as usize]
    }
}

impl fmt::Debug for Vfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vfs").field("n_files", &self.data.len()).finish()
    }
}
