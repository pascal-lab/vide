use std::mem;

use rustc_hash::FxHashMap;
use vfs::VfsPath;

#[derive(Default, Clone)]
pub(crate) struct MemDocs {
    mem_docs: FxHashMap<VfsPath, DocumentData>,
    added_or_removed: bool,
}

impl MemDocs {
    pub(crate) fn contains(&self, path: &VfsPath) -> bool {
        self.mem_docs.contains_key(path)
    }

    pub(crate) fn insert(&mut self, path: VfsPath, data: DocumentData) -> Result<(), ()> {
        self.added_or_removed = true;
        match self.mem_docs.insert(path, data) {
            Some(_) => Err(()),
            None => Ok(()),
        }
    }

    pub(crate) fn remove(&mut self, path: &VfsPath) -> Result<(), ()> {
        self.added_or_removed = true;
        match self.mem_docs.remove(path) {
            Some(_) => Ok(()),
            None => Err(()),
        }
    }

    pub(crate) fn get(&self, path: &VfsPath) -> Option<&DocumentData> {
        self.mem_docs.get(path)
    }

    pub(crate) fn get_mut(&mut self, path: &VfsPath) -> Option<&mut DocumentData> {
        // NB: don't set `self.added_or_removed` here, as that purposefully only
        // tracks changes to the key set.
        self.mem_docs.get_mut(path)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &VfsPath> {
        self.mem_docs.keys()
    }

    pub(crate) fn take_changes(&mut self) -> bool {
        mem::replace(&mut self.added_or_removed, false)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DocumentData {
    pub(crate) version: i32,
}

impl DocumentData {
    pub(crate) fn new(version: i32) -> Self {
        DocumentData { version }
    }
}
