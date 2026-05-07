use rustc_hash::FxHashMap;
use vfs::{FileId, VfsPath};

#[derive(Debug, Clone)]
pub(crate) struct DocumentData {
    pub(crate) version: i32,
    pub(crate) data: String,
}

// Files managed by client via notifications
#[derive(Default, Clone)]
pub(crate) struct MemDocs {
    mem_docs: FxHashMap<VfsPath, DocumentData>,
    path_by_file_id: FxHashMap<FileId, VfsPath>,
    file_id_by_path: FxHashMap<VfsPath, FileId>,
}

impl MemDocs {
    pub(crate) fn contains(&self, path: &VfsPath) -> bool {
        self.mem_docs.contains_key(path)
    }

    pub(crate) fn insert(
        &mut self,
        file_id: FileId,
        path: VfsPath,
        data: DocumentData,
    ) -> Option<DocumentData> {
        if let Some(old_path) = self.path_by_file_id.insert(file_id, path.clone()) {
            self.file_id_by_path.remove(&old_path);
        }
        if let Some(old_file_id) = self.file_id_by_path.insert(path.clone(), file_id) {
            self.path_by_file_id.remove(&old_file_id);
        }
        self.mem_docs.insert(path, data)
    }

    pub(crate) fn remove(&mut self, path: &VfsPath) -> Option<DocumentData> {
        if let Some(file_id) = self.file_id_by_path.remove(path) {
            self.path_by_file_id.remove(&file_id);
        }
        self.mem_docs.remove(path)
    }

    pub(crate) fn get_mut(&mut self, path: &VfsPath) -> Option<&mut DocumentData> {
        self.mem_docs.get_mut(path)
    }

    pub(crate) fn version(&self, file_id: FileId) -> Option<i32> {
        let path = self.path_by_file_id.get(&file_id)?;
        Some(self.mem_docs.get(path)?.version)
    }

    pub(crate) fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.file_id_by_path.get(path).copied()
    }

    pub(crate) fn file_ids(&self) -> impl Iterator<Item = FileId> + '_ {
        self.path_by_file_id.keys().copied()
    }
}
