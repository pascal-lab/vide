use rustc_hash::FxHashMap;
use vfs::{FileId, VfsPath};

#[derive(Debug, Clone)]
pub(crate) struct OpenBuffer {
    pub(crate) path: VfsPath,
    pub(crate) data: String,
}

impl OpenBuffer {
    pub(crate) fn new(path: VfsPath, data: String) -> Self {
        Self { path, data }
    }
}

/// One open URI spelling and its LSP document version.
#[derive(Debug, Clone)]
pub(crate) struct OpenDocument {
    pub(crate) path: VfsPath,
    pub(crate) version: i32,
}

#[derive(Debug, Clone)]
struct OpenDocumentAlias {
    file_id: FileId,
    version: i32,
    buffer_attached: bool,
}

// Files managed by client via textDocument/didOpen and textDocument/didClose.
//
// MemDocs keeps one analysis buffer per canonical FileId and one LSP document
// version per open URI spelling. Multiple open URI aliases therefore share
// text when their didOpen contents agree, but their version numbers remain
// URI-local. An alias opened with divergent text is tracked for close/version
// bookkeeping but is detached from the canonical analysis buffer; supporting
// multiple unsaved alias buffers is a separate design.
//
// TODO: Allow detached aliases to reattach on a full-document change that
// matches the canonical analysis buffer, or expose an explicit unsupported
// multi-alias-unsaved-buffer status to the client.
#[derive(Default, Clone)]
pub(crate) struct MemDocs {
    buffers: FxHashMap<FileId, OpenBuffer>,
    open_paths: FxHashMap<VfsPath, OpenDocumentAlias>,
}

impl MemDocs {
    pub(crate) fn contains_path(&self, path: &VfsPath) -> bool {
        self.open_paths.contains_key(path)
    }

    pub(crate) fn contains_file_id(&self, file_id: FileId) -> bool {
        self.buffers.contains_key(&file_id)
    }

    pub(crate) fn insert(
        &mut self,
        file_id: FileId,
        path: VfsPath,
        version: i32,
        data: String,
    ) -> bool {
        if self.open_paths.get(&path).is_some_and(|alias| alias.file_id == file_id) {
            return true;
        }

        let buffer_attached = self.buffers.get(&file_id).is_none_or(|buffer| buffer.data == data);
        if let Some(old_alias) = self
            .open_paths
            .insert(path.clone(), OpenDocumentAlias { file_id, version, buffer_attached })
            && old_alias.file_id != file_id
        {
            self.reconcile_buffer_path(old_alias.file_id);
        }

        if self.buffers.contains_key(&file_id) {
            // TODO: Support multiple independent unsaved buffers for URI
            // aliases of the same physical file. That is intentionally not
            // done in this T1 change; VFS and analysis currently have one text
            // slot per canonical FileId.
            return false;
        }

        self.buffers.insert(file_id, OpenBuffer::new(path, data));
        false
    }

    pub(crate) fn remove_path(&mut self, path: &VfsPath) -> bool {
        let Some(alias) = self.open_paths.remove(path) else {
            return false;
        };
        self.reconcile_buffer_path(alias.file_id);
        true
    }

    pub(crate) fn remap_file_id(&mut self, from: FileId, to: FileId) {
        if from == to {
            return;
        }

        let duplicate_buffer = self.buffers.remove(&from);
        let attaches_to_existing_buffer = duplicate_buffer.as_ref().is_none_or(|buffer| {
            self.buffers.get(&to).is_none_or(|existing| existing.data == buffer.data)
        });
        for alias in self.open_paths.values_mut() {
            if alias.file_id == from {
                alias.file_id = to;
                alias.buffer_attached &= attaches_to_existing_buffer;
            }
        }
        if !self.buffers.contains_key(&to)
            && let Some(buffer) = duplicate_buffer
        {
            self.buffers.insert(to, buffer);
        }
        self.reconcile_buffer_path(to);
    }

    pub(crate) fn text_for_change(&self, path: &VfsPath, file_id: FileId) -> Option<&str> {
        let alias = self.open_paths.get(path)?;
        if alias.file_id != file_id || !alias.buffer_attached {
            return None;
        }
        self.text(file_id)
    }

    pub(crate) fn apply_change(
        &mut self,
        path: &VfsPath,
        file_id: FileId,
        version: i32,
        data: Option<String>,
    ) -> bool {
        let Some(alias) = self.open_paths.get_mut(path) else {
            return false;
        };
        if alias.file_id != file_id || !alias.buffer_attached {
            return false;
        }
        alias.version = version;
        if let Some(data) = data
            && let Some(buffer) = self.buffers.get_mut(&file_id)
        {
            buffer.data = data;
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn version(&self, file_id: FileId) -> Option<i32> {
        let path = &self.buffers.get(&file_id)?.path;
        self.version_for_path(path)
    }

    pub(crate) fn text(&self, file_id: FileId) -> Option<&str> {
        Some(self.buffers.get(&file_id)?.data.as_str())
    }

    pub(crate) fn open_documents(&self, file_id: FileId) -> Vec<OpenDocument> {
        let mut documents = self
            .open_paths
            .iter()
            .filter(|(_, alias)| alias.file_id == file_id && alias.buffer_attached)
            .map(|(path, alias)| OpenDocument { path: path.clone(), version: alias.version })
            .collect::<Vec<_>>();
        documents.sort_unstable_by(|lhs, rhs| lhs.path.cmp(&rhs.path));
        documents
    }

    pub(crate) fn version_for_path(&self, path: &VfsPath) -> Option<i32> {
        Some(self.open_paths.get(path)?.version)
    }

    pub(crate) fn file_id(&self, path: &VfsPath) -> Option<FileId> {
        Some(self.open_paths.get(path)?.file_id)
    }

    pub(crate) fn file_ids(&self) -> impl Iterator<Item = FileId> + '_ {
        self.buffers.keys().copied()
    }

    fn reconcile_buffer_path(&mut self, file_id: FileId) {
        let Some(current_path) = self.buffers.get(&file_id).map(|buffer| buffer.path.clone())
        else {
            return;
        };
        if self
            .open_paths
            .get(&current_path)
            .is_some_and(|alias| alias.file_id == file_id && alias.buffer_attached)
        {
            return;
        }

        let replacement_path = self.open_paths.iter().find_map(|(path, alias)| {
            (alias.file_id == file_id && alias.buffer_attached).then(|| path.clone())
        });
        match replacement_path {
            Some(path) => {
                if let Some(buffer) = self.buffers.get_mut(&file_id) {
                    buffer.path = path;
                }
            }
            None => {
                self.buffers.remove(&file_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;

    use super::*;

    #[test]
    fn mem_docs_alias_matrix() {
        let canonical_path = VfsPath::new_virtual_path("/workspace/top.sv".to_owned());
        let alias_path = VfsPath::new_virtual_path("/alias/top.sv".to_owned());
        let duplicate = FileId(1);
        let canonical = FileId(0);
        let mut report = String::new();

        let path = canonical_path.clone();
        let mut docs = MemDocs::default();
        docs.insert(duplicate, path.clone(), 7, "module top; endmodule\n".to_owned());
        docs.remap_file_id(duplicate, canonical);
        render_case(
            &mut report,
            "remap_file_id_moves_open_document_to_canonical_id",
            &docs,
            &[canonical, duplicate],
            &[&canonical_path, &alias_path],
            canonical,
        );

        let mut docs = MemDocs::default();
        docs.insert(canonical, canonical_path.clone(), 1, "canonical text".to_owned());
        docs.insert(duplicate, alias_path.clone(), 2, "alias text".to_owned());
        docs.remap_file_id(duplicate, canonical);
        render_case(
            &mut report,
            "remap_file_id_preserves_existing_canonical_document",
            &docs,
            &[canonical, duplicate],
            &[&canonical_path, &alias_path],
            canonical,
        );

        let mut docs = MemDocs::default();
        docs.insert(canonical, canonical_path.clone(), 1, "canonical text".to_owned());
        docs.insert(canonical, alias_path.clone(), 2, "canonical text".to_owned());
        let removed = docs.remove_path(&alias_path);
        writeln!(&mut report, "remove_path_closes_alias_without_removing_open_document:").unwrap();
        writeln!(&mut report, "  removed: {removed}").unwrap();
        render_docs(&mut report, &docs, &[canonical], &[&canonical_path, &alias_path], canonical);

        let mut docs = MemDocs::default();
        docs.insert(canonical, canonical_path.clone(), 1, "canonical text".to_owned());
        docs.insert(canonical, alias_path.clone(), 2, "canonical text".to_owned());
        let removed = docs.remove_path(&canonical_path);
        writeln!(&mut report, "remove_path_promotes_remaining_alias_when_owner_path_closes:")
            .unwrap();
        writeln!(&mut report, "  removed: {removed}").unwrap();
        render_docs(&mut report, &docs, &[canonical], &[&canonical_path, &alias_path], canonical);

        let mut docs = MemDocs::default();
        docs.insert(canonical, canonical_path.clone(), 1, "module top; endmodule\n".to_owned());
        docs.insert(canonical, alias_path.clone(), 7, "module top; endmodule\n".to_owned());
        let mut data = docs.text_for_change(&alias_path, canonical).unwrap().to_owned();
        data.push_str("// alias edit\n");
        let applied = docs.apply_change(&alias_path, canonical, 8, Some(data));
        writeln!(&mut report, "changes_update_only_the_changed_uri_version:").unwrap();
        writeln!(&mut report, "  applied: {applied}").unwrap();
        render_docs(&mut report, &docs, &[canonical], &[&canonical_path, &alias_path], canonical);

        let mut docs = MemDocs::default();
        docs.insert(canonical, canonical_path.clone(), 1, "canonical text".to_owned());
        docs.insert(canonical, alias_path.clone(), 7, "alias text".to_owned());
        let applied =
            docs.apply_change(&alias_path, canonical, 8, Some("new alias text".to_owned()));
        writeln!(&mut report, "divergent_alias_open_is_detached_from_canonical_buffer:").unwrap();
        writeln!(&mut report, "  applied: {applied}").unwrap();
        render_docs(&mut report, &docs, &[canonical], &[&canonical_path, &alias_path], canonical);

        let mut docs = MemDocs::default();
        docs.insert(canonical, canonical_path.clone(), 1, "module top; endmodule\n".to_owned());
        docs.insert(canonical, alias_path.clone(), 7, "module top; endmodule\n".to_owned());
        render_case(
            &mut report,
            "open_documents_return_every_uri_version",
            &docs,
            &[canonical],
            &[&canonical_path, &alias_path],
            canonical,
        );

        insta::assert_snapshot!(report);
    }

    fn render_case(
        report: &mut String,
        name: &str,
        docs: &MemDocs,
        file_ids: &[FileId],
        paths: &[&VfsPath],
        open_docs_file_id: FileId,
    ) {
        writeln!(report, "{name}:").unwrap();
        render_docs(report, docs, file_ids, paths, open_docs_file_id);
    }

    fn render_docs(
        report: &mut String,
        docs: &MemDocs,
        file_ids: &[FileId],
        paths: &[&VfsPath],
        open_docs_file_id: FileId,
    ) {
        writeln!(report, "  file_ids:").unwrap();
        for file_id in file_ids {
            writeln!(report, "    {:?}: contains={}", file_id, docs.contains_file_id(*file_id))
                .unwrap();
            writeln!(report, "      version={:?}", docs.version(*file_id)).unwrap();
            match docs.buffers.get(file_id) {
                Some(buffer) => {
                    writeln!(report, "      buffer path={:?} data={:?}", buffer.path, buffer.data)
                        .unwrap()
                }
                None => writeln!(report, "      buffer=<none>").unwrap(),
            }
        }

        writeln!(report, "  paths:").unwrap();
        for path in paths {
            writeln!(report, "    {path:?}:").unwrap();
            writeln!(report, "      file_id={:?}", docs.file_id(path)).unwrap();
            writeln!(report, "      version={:?}", docs.version_for_path(path)).unwrap();
            writeln!(
                report,
                "      text_for_change={:?}",
                docs.text_for_change(path, open_docs_file_id)
            )
            .unwrap();
        }

        let documents = docs
            .open_documents(open_docs_file_id)
            .into_iter()
            .map(|document| (document.path, document.version))
            .collect::<Vec<_>>();
        writeln!(report, "  open_documents={documents:?}").unwrap();
    }
}
