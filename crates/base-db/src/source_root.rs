use vfs::{FileId, FileSet, FileSetConfig, Vfs, VfsPath, anchored_path::AnchoredPath};

use crate::source_db::SourceFileKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceRootId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceRootRole {
    Local,
    IndexOnly,
    Library,
    Ignored,
}

impl SourceRootRole {
    pub fn is_library(self) -> bool {
        matches!(self, SourceRootRole::Library)
    }

    pub fn is_index_only(self) -> bool {
        matches!(self, SourceRootRole::IndexOnly)
    }

    pub fn is_ignored(self) -> bool {
        matches!(self, SourceRootRole::Ignored)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRoot {
    role: SourceRootRole,
    source_files: Option<Vec<FileId>>,
    file_set: FileSet,
}

impl SourceRoot {
    pub fn new(role: SourceRootRole, file_set: FileSet) -> SourceRoot {
        SourceRoot { role, source_files: None, file_set }
    }

    pub fn with_source_files(
        role: SourceRootRole,
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot { role, source_files: Some(source_files), file_set }
    }

    pub fn new_local(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::Local, file_set)
    }

    pub fn new_library(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::Library, file_set)
    }

    pub fn new_index_only(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::IndexOnly, file_set)
    }

    pub fn new_ignored(file_set: FileSet) -> SourceRoot {
        SourceRoot::new(SourceRootRole::Ignored, file_set)
    }

    pub fn new_local_with_source_files(file_set: FileSet, source_files: Vec<FileId>) -> SourceRoot {
        SourceRoot::with_source_files(SourceRootRole::Local, file_set, source_files)
    }

    pub fn new_library_with_source_files(
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot::with_source_files(SourceRootRole::Library, file_set, source_files)
    }

    pub fn new_index_only_with_source_files(
        file_set: FileSet,
        source_files: Vec<FileId>,
    ) -> SourceRoot {
        SourceRoot::with_source_files(SourceRootRole::IndexOnly, file_set, source_files)
    }

    pub fn role(&self) -> SourceRootRole {
        self.role
    }

    pub fn is_library(&self) -> bool {
        self.role.is_library()
    }

    pub fn is_index_only(&self) -> bool {
        self.role.is_index_only()
    }

    pub fn is_ignored(&self) -> bool {
        self.role.is_ignored()
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
    pub fileset_roles: Vec<SourceRootRole>,
}

impl SourceRootConfig {
    pub fn partition(&self, vfs: &Vfs) -> Vec<SourceRoot> {
        self.fileset_config
            .partition_with_source(vfs)
            .into_iter()
            .enumerate()
            .map(|(idx, partition)| {
                let file_set = partition.file_set;
                let role = self.fileset_roles.get(idx).copied().unwrap_or(SourceRootRole::Library);
                let source_files =
                    partition.source_files.map(|source_files| source_files.into_iter().collect());
                if role.is_ignored() {
                    return SourceRoot::new(role, file_set);
                }

                match source_files {
                    Some(source_files) => {
                        SourceRoot::with_source_files(role, file_set, source_files)
                    }
                    None => SourceRoot::new(role, file_set),
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
            fileset_roles: vec![SourceRootRole::Library, SourceRootRole::Ignored],
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
    fn index_only_root_preserves_file_kind() {
        let mut file_set = FileSet::default();
        let file_id = FileId(0);
        file_set.insert(file_id, VfsPath::new_virtual_path("/indexed/file.sv".into()));
        let root = SourceRoot::new_index_only(file_set);

        assert_eq!(root.role(), SourceRootRole::IndexOnly);
        assert!(root.is_index_only());
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
