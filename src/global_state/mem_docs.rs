use rustc_hash::FxHashMap;
use vfs::VfsPath;

#[derive(Debug, Clone)]
pub(crate) struct DocumentData {
    pub(crate) version: i32,
    pub(crate) data: String,
}

// Files managed by client via notifications
#[derive(Default, Clone)]
pub(crate) struct MemDocs {
    mem_docs: FxHashMap<VfsPath, DocumentData>,
}

impl MemDocs {
    pub(crate) fn contains(&self, path: &VfsPath) -> bool {
        self.mem_docs.contains_key(path)
    }

    pub(crate) fn insert(&mut self, path: VfsPath, data: DocumentData) -> Option<DocumentData> {
        self.mem_docs.insert(path, data)
    }

    pub(crate) fn remove(&mut self, path: &VfsPath) -> Option<DocumentData> {
        self.mem_docs.remove(path)
    }

    pub(crate) fn get(&self, path: &VfsPath) -> Option<&DocumentData> {
        self.mem_docs.get(path)
    }

    pub(crate) fn get_mut(&mut self, path: &VfsPath) -> Option<&mut DocumentData> {
        self.mem_docs.get_mut(path)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &VfsPath> {
        self.mem_docs.keys()
    }
}
