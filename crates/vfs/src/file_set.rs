use nohash_hasher::IntMap;
use rustc_hash::FxHashMap;

use crate::{
    vfs::{AnchoredPath, FileId},
    vfs_path::VfsPath,
};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct FileSet {
    files: FxHashMap<VfsPath, FileId>,
    paths: IntMap<FileId, VfsPath>,
}

impl FileSet {
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn get_file(&self, path: &VfsPath) -> Option<&FileId> {
        self.files.get(path)
    }

    pub fn get_path(&self, file: &FileId) -> Option<&VfsPath> {
        self.paths.get(file)
    }

    pub fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        let mut base = self.paths[&path.anchor_id].clone();
        base.pop();
        let path = base.join(path.path)?;
        self.files.get(&path).copied()
    }

    pub fn insert(&mut self, file_id: FileId, path: VfsPath) {
        self.files.insert(path.clone(), file_id);
        self.paths.insert(file_id, path);
    }

    pub fn iter(&self) -> impl Iterator<Item = FileId> + '_ {
        self.paths.keys().copied()
    }
}
