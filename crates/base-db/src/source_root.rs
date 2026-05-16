use utils::paths::AbsPathBuf;
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
    source_paths: Option<Vec<AbsPathBuf>>,
    file_set: FileSet,
}

impl SourceRoot {
    pub fn new_local(file_set: FileSet) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Local, source_paths: None, file_set }
    }

    pub fn new_library(file_set: FileSet) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Library, source_paths: None, file_set }
    }

    pub fn new_ignored(file_set: FileSet) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Ignored, source_paths: None, file_set }
    }

    pub fn new_local_with_source_paths(
        file_set: FileSet,
        source_paths: Vec<AbsPathBuf>,
    ) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Local, source_paths: Some(source_paths), file_set }
    }

    pub fn new_library_with_source_paths(
        file_set: FileSet,
        source_paths: Vec<AbsPathBuf>,
    ) -> SourceRoot {
        SourceRoot { role: SourceRootRole::Library, source_paths: Some(source_paths), file_set }
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
        let Some(abs_path) = path.as_abs_path() else {
            return SourceFileKind::from_path(path);
        };
        let kind = SourceFileKind::from_path(path);
        let Some(source_paths) = &self.source_paths else {
            return kind;
        };
        if matches!(kind, SourceFileKind::LibraryMap)
            || source_paths.iter().any(|source_path| abs_path.starts_with(source_path))
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
    pub source_paths_by_fileset: Vec<Vec<AbsPathBuf>>,
}

impl SourceRootConfig {
    pub fn partition(&self, vfs: &Vfs) -> Vec<SourceRoot> {
        self.fileset_config
            .partition(vfs)
            .into_iter()
            .enumerate()
            .map(|(idx, file_set)| {
                let source_paths = self.source_paths_by_fileset.get(idx).cloned();
                if self.ignored_filesets.contains(&idx) {
                    return SourceRoot::new_ignored(file_set);
                }
                match (self.local_filesets.contains(&idx), source_paths) {
                    (true, Some(source_paths)) => {
                        SourceRoot::new_local_with_source_paths(file_set, source_paths)
                    }
                    (true, None) => SourceRoot::new_local(file_set),
                    (false, Some(source_paths)) => {
                        SourceRoot::new_library_with_source_paths(file_set, source_paths)
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
            source_paths_by_fileset: Vec::new(),
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
}
