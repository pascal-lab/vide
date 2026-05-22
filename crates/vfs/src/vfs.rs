use std::{fmt, hash::BuildHasherDefault, mem};

use indexmap::IndexSet;
use rustc_hash::FxHasher;
use triomphe::Arc;
use utils::lines::LineEnding;

use crate::{loader::LoadResult, vfs_path::VfsPath};

#[derive(Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct FileId(pub u32);

impl nohash_hasher::IsEnabled for FileId {}

#[derive(Default)]
pub struct Vfs {
    paths: IndexSet<VfsPath, BuildHasherDefault<FxHasher>>,
    file_states: Vec<FileState>,
    changes: Vec<ChangedFile>,
}

#[derive(PartialEq, PartialOrd, Clone)]
pub enum FileState {
    Exists(String, LineEnding),
    Deleted,
}

#[derive(Debug)]
pub struct ChangedFile {
    pub file_id: FileId,
    pub change_kind: ChangeKind,
}

impl ChangedFile {
    pub fn is_created_or_deleted(&self) -> bool {
        matches!(self.change_kind, ChangeKind::Create(_, _) | ChangeKind::Delete)
    }

    pub fn text(&self) -> Option<Arc<str>> {
        match &self.change_kind {
            ChangeKind::Create(text, _) | ChangeKind::Modify(text, _) => Some(text.clone()),
            ChangeKind::Delete => None,
        }
    }

    pub fn ending(&self) -> Option<LineEnding> {
        match &self.change_kind {
            ChangeKind::Create(_, ending) | ChangeKind::Modify(_, ending) => Some(*ending),
            ChangeKind::Delete => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ChangeKind {
    Create(Arc<str>, LineEnding),
    Modify(Arc<str>, LineEnding),
    Delete,
}

impl Vfs {
    pub fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.paths.get_index_of(path).map(|id| FileId(id as u32)).filter(|id| self.exists(*id))
    }

    pub fn file_path(&self, file_id: FileId) -> Option<&VfsPath> {
        self.paths.get_index(file_id.0 as usize)
    }

    pub fn line_ending(&self, file_id: FileId) -> Option<LineEnding> {
        match self.file_state(file_id)? {
            FileState::Exists(_, ending) => Some(*ending),
            FileState::Deleted => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &VfsPath)> + '_ {
        (0..self.file_states.len())
            .map(|it| FileId(it as u32))
            .filter(move |&file_id| self.exists(file_id))
            .filter_map(move |file_id| self.file_path(file_id).map(|path| (file_id, path)))
    }

    pub fn set_file_contents(&mut self, path: &VfsPath, contents: LoadResult) {
        let file_id = self.file_id_or_alloc(path);
        use FileState::*;
        use LoadResult::*;
        let Some(state) = self.file_states.get_mut(file_id.0 as usize) else {
            return;
        };
        let change_kind = match contents {
            Loaded(new, new_ending) => match state {
                Exists(old, _) if *old == new => return,
                Exists(_, _) => {
                    let change_kind = ChangeKind::Modify(Arc::from(new.as_str()), new_ending);
                    *state = Exists(new, new_ending);
                    change_kind
                }
                Deleted => {
                    let change_kind = ChangeKind::Create(Arc::from(new.as_str()), new_ending);
                    *state = Exists(new, new_ending);
                    change_kind
                }
            },
            LoadError => match state {
                Exists(_, _) => {
                    *state = Deleted;
                    ChangeKind::Delete
                }
                Deleted => return,
            },
            DecodeError => return,
        };

        let changed_file = ChangedFile { file_id, change_kind };
        self.changes.push(changed_file);
    }

    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    pub fn take_changes(&mut self) -> Vec<ChangedFile> {
        mem::take(&mut self.changes)
    }

    pub fn exists(&self, file_id: FileId) -> bool {
        matches!(self.file_state(file_id), Some(FileState::Exists(_, _)))
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

    fn file_state(&self, file_id: FileId) -> Option<&FileState> {
        self.file_states.get(file_id.0 as usize)
    }
}

impl fmt::Debug for Vfs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vfs").field("n_files", &self.file_states.len()).finish()
    }
}

#[cfg(test)]
mod tests {
    use utils::lines::LineEnding;

    use super::*;

    #[test]
    fn load_error_marks_existing_file_deleted() {
        let mut vfs = Vfs::default();
        let path = VfsPath::new_virtual_path("/workspace/vizsla.toml".to_owned());

        vfs.set_file_contents(
            &path,
            LoadResult::Loaded("sources = []\n".to_owned(), LineEnding::Unix),
        );
        let file_id = vfs.file_id(&path).unwrap();
        vfs.take_changes();

        vfs.set_file_contents(&path, LoadResult::LoadError);

        assert!(!vfs.exists(file_id));
        assert_eq!(vfs.file_id(&path), None);
        let changes = vfs.take_changes();
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0].change_kind, ChangeKind::Delete));
    }
}
