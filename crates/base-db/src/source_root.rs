use vfs::{FileId, FileSet, FileSetConfig, Vfs, VfsPath, anchored_path::AnchoredPath};

use crate::source_db::SourceFileKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceRootId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRootRole {
    Local,
    Library,
    Ignored,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRoot {
    role: SourceRootRole,
    source_files: Option<Vec<FileId>>,
    file_set: FileSet,
}

impl SourceRoot {
    pub fn new_local(file_set: FileSet) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Local, source_files: None, file_set }
    }

    pub fn new_library(file_set: FileSet) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Library, source_files: None, file_set }
    }

    pub fn new_ignored(file_set: FileSet) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Ignored, source_files: None, file_set }
    }

    pub fn new_local_with_source_files(file_set: FileSet, source_files: Vec<FileId>) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Local, source_files: Some(source_files), file_set }
    }

    pub fn new_library_with_source_files(
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Library, source_files: Some(source_files), file_set }
    }

    pub fn role(&self) -> SourceRootRole {
        self.role
    }

    pub fn is_library(&self) -> bool {
        matches!(self.role, SourceRootRole::Library)
    }

    pub fn is_ignored(&self) -> bool {
        matches!(self.role, SourceRootRole::Ignored)
    }

    pub fn path_for_file(&self, file: &FileId) -> Option<&VfsPath> {
        self.file_set.get_path(file)
    }

    pub fn file_for_path(&self, path: &VfsPath) -> Option<&FileId> {
        self.file_set.get_file(path)
    }

    pub fn resolve_path(&self, path: AnchoredPath<'_>) -> Option<FileId> {
        self.file_set.resolve_path(path)
    }

    pub fn iter(&self) -> impl Iterator<Item = FileId> + '_ {
        self.file_set.iter()
    }

    pub fn file_kind(&self, file: &FileId) -> SourceFileKind {
        let Some(path) = self.path_for_file(file) else {
            return SourceFileKind::default();
        };
        let kind = SourceFileKind::from_path(path);
        let Some(source_files) = &self.source_files else {
            return kind;
        };
        if matches!(kind, SourceFileKind::LibraryMap | SourceFileKind::ProjectManifest)
            || source_files.contains(file)
        {
            kind
        } else {
            SourceFileKind::IncludeHeader
        }
    }
}

#[derive(Default, Debug)]
pub struct SourceRootConfig {
    pub fileset_config: FileSetConfig,
    pub local_filesets: Vec<usize>,
    pub ignored_filesets: Vec<usize>,
}

impl SourceRootConfig {
    pub fn partition(&self, vfs: &Vfs) -> Vec<SourceRoot> {
        self.fileset_config
            .partition_with_source(vfs)
            .into_iter()
            .enumerate()
            .map(|(idx, partition)| {
                let file_set = partition.file_set;
                let source_files =
                    partition.source_files.map(|source_files| source_files.into_iter().collect());
                if self.ignored_filesets.contains(&idx) {
                    return SourceRoot::new_ignored(file_set);
                }
                match (self.local_filesets.contains(&idx), source_files) {
                    (true, Some(source_files)) => {
                        SourceRoot::new_local_with_source_files(file_set, source_files)
                    }
                    (true, None) => SourceRoot::new_local(file_set),
                    (false, Some(source_files)) => {
                        SourceRoot::new_library_with_source_files(file_set, source_files)
                    }
                    (false, None) => SourceRoot::new_library(file_set),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_local_fileset_without_source_paths_remains_library() {
        let mut builder = FileSetConfig::builder();
        builder.add_file_set(vec![VfsPath::new_virtual_path("/lib".into())]);
        let config = SourceRootConfig {
            fileset_config: builder.build(),
            local_filesets: Vec::new(),
            ignored_filesets: vec![1],
        };
        let roots = config.partition(&Vfs::default());

        assert_eq!(roots[0].role(), SourceRootRole::Library);
        assert_eq!(roots[1].role(), SourceRootRole::Ignored);
    }

    #[test]
    fn ignored_root_preserves_file_kind() {
        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new_virtual_path("/ignored/file.sv".into()));
        let root = SourceRoot::new_ignored(file_set);

        assert_eq!(root.role(), SourceRootRole::Ignored);
        assert_eq!(root.file_kind(&file_id), SourceFileKind::SystemVerilog);
    }

    #[test]
    fn source_filtered_root_preserves_project_manifest_kind() {
        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new_virtual_path("/root/vizsla.toml".into()));
        let root = SourceRoot::new_local_with_source_files(file_set, Vec::new());

        assert_eq!(root.file_kind(&file_id), SourceFileKind::ProjectManifest);
    }
}
