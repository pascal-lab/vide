use rustc_hash::FxHashMap;
use vfs::{FileId, VfsPath};

#[derive(Debug, Clone)]
pub(crate) struct OpenDocument {
    pub(crate) path: VfsPath,
    pub(crate) version: i32,
    pub(crate) data: String,
}

impl OpenDocument {
    pub(crate) fn new(path: VfsPath, version: i32, data: String) -> Self {
        Self { path, version, data }
    }
}

// Files managed by client via textDocument/didOpen and textDocument/didClose.
#[derive(Default, Clone)]
pub(crate) struct MemDocs {
    docs: FxHashMap<FileId, OpenDocument>,
    file_id_by_path: FxHashMap<VfsPath, FileId>,
}

impl MemDocs {
    pub(crate) fn contains_path(&self, path: &VfsPath) -> bool {
        self.file_id_by_path.contains_key(path)
    }

    pub(crate) fn insert(
        &mut self,
        file_id: FileId,
        path: VfsPath,
        version: i32,
        data: String,
    ) -> Option<OpenDocument> {
        if let Some(old_file_id) = self.file_id_by_path.insert(path.clone(), file_id)
            && old_file_id != file_id
        {
            self.docs.remove(&old_file_id);
        }

        if let Some(old_doc) =
            self.docs.insert(file_id, OpenDocument::new(path.clone(), version, data))
        {
            self.file_id_by_path.remove(&old_doc.path);
            self.file_id_by_path.insert(path, file_id);
            return Some(old_doc);
        }

        None
    }

    pub(crate) fn remove_path(&mut self, path: &VfsPath) -> Option<OpenDocument> {
        let file_id = self.file_id_by_path.remove(path)?;
        self.docs.remove(&file_id)
    }

    pub(crate) fn get_mut_by_path(&mut self, path: &VfsPath) -> Option<&mut OpenDocument> {
        let file_id = self.file_id_by_path.get(path)?;
        self.docs.get_mut(file_id)
    }

    pub(crate) fn version(&self, file_id: FileId) -> Option<i32> {
        Some(self.docs.get(&file_id)?.version)
    }

    pub(crate) fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        self.file_id_by_path.get(path).copied()
    }

    pub(crate) fn file_ids(&self) -> impl Iterator<Item = FileId> + '_ {
        self.docs.keys().copied()
    }
}
